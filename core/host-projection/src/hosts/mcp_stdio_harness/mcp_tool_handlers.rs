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
                tracing::warn!(
                    "CompositeRegistry: duplicate tool name '{name}' \
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
// RoutingTools (4 tools)
// ---------------------------------------------------------------------------

pub struct RoutingTools;
impl ToolHandler for RoutingTools {
    fn tool_names(&self) -> &[&'static str] {
        &["skill_route", "skill_search", "skill_read", "skill_route_status"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "skill_route" => tool_skill_route(args, &ctx.repo_root, &ctx.host_id),
            "skill_search" => tool_skill_search(args, &ctx.repo_root, &ctx.host_id),
            "skill_read" => tool_skill_read(args, &ctx.repo_root),
            "skill_route_status" => tool_skill_route_status(&ctx.repo_root),
            _ => Err(format!("RoutingTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// GoalTools (2 tools: goal_state_read, goal_state_manage)
// ---------------------------------------------------------------------------

pub struct GoalTools;
impl ToolHandler for GoalTools {
    fn tool_names(&self) -> &[&'static str] {
        &["goal_state_read", "goal_state_manage"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "goal_state_read" => tool_goal_state_read(args, &ctx.repo_root),
            "goal_state_manage" => tool_goal_state_manage(args, &ctx.repo_root, &ctx.connection_session_id),
            _ => Err(format!("GoalTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// CloseoutTools (2 tools: closeout_gate, closeout_record_write)
// ---------------------------------------------------------------------------

pub struct CloseoutTools;
impl ToolHandler for CloseoutTools {
    fn tool_names(&self) -> &[&'static str] {
        &["closeout_gate", "closeout_record_write"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "closeout_gate" => tool_closeout_gate(args, &ctx.repo_root, &ctx.host_id),
            "closeout_record_write" => tool_closeout_record_write(args, &ctx.repo_root, &ctx.host_id),
            _ => Err(format!("CloseoutTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// FrameworkTools (2 tools: record_evidence, session_checkpoint)
// ---------------------------------------------------------------------------

pub struct FrameworkTools;
impl ToolHandler for FrameworkTools {
    fn tool_names(&self) -> &[&'static str] {
        &["record_evidence", "session_checkpoint"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "record_evidence" => tool_record_evidence(args, &ctx.repo_root),
            "session_checkpoint" => tool_session_checkpoint(args, &ctx.repo_root),
            _ => Err(format!("FrameworkTools: unknown tool: {tool_name}")),
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
            "route_tool" => tool_route_tool(args, &ctx.repo_root, &ctx.host_id),
            "search_tools" => tool_search_tools(args, &ctx.repo_root, &ctx.host_id),
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
        &["task_create", "task_list", "task_complete", "task_focus", "task_chain_advance"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "task_create" => Ok(tool_task_create(args, &ctx.repo_root)?),
            "task_list" => Ok(tool_task_list(&ctx.repo_root)?),
            "task_complete" => Ok(tool_task_complete(args, &ctx.repo_root)?),
            "task_focus" => Ok(tool_task_focus(args, &ctx.repo_root)?),
            "task_chain_advance" => Ok(tool_task_chain_advance(args, &ctx.repo_root)?),
            _ => Err(format!("TaskCrudTools: unknown tool: {tool_name}")),
        }
    }
}

/// Resolve the tool registry path. Uses hooks, falls back to repo_root default.
fn resolve_tool_registry_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    mcp_tool_registry::resolve_tool_registry_path()
        .unwrap_or_else(|| repo_root.join(framework_kernel::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH))
}

// ---------------------------------------------------------------------------
// OrchestratorTools (14 tools: team, agent, worker operations)
// ---------------------------------------------------------------------------

pub struct OrchestratorTools;
impl ToolHandler for OrchestratorTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "orchestrator_team_create",
            "orchestrator_team_add_member",
            "orchestrator_team_remove_member",
            "orchestrator_team_complete",
            "orchestrator_team_send_message",
            "orchestrator_team_read_messages",
            "orchestrator_team_alive_members",
            "orchestrator_team_list",
            "orchestrator_agent_register",
            "orchestrator_agent_unregister",
            "orchestrator_agent_list_running",
            "orchestrator_worker_launch",
            "orchestrator_worker_list",
            "orchestrator_worker_terminate",
        ]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        let operation = orchestrator_operation_for_tool(tool_name)?;
        let mut payload = args.clone();
        payload["operation"] = json!(operation);
        payload["state_path"] = json!(format!(
            "{}/artifacts/orchestrator/state.json",
            ctx.repo_root.display()
        ));
        let result = framework_kernel::runtime_hooks::hooks().handle_orchestrator_operation(payload)?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

fn orchestrator_operation_for_tool(tool_name: &str) -> Result<&'static str, String> {
    match tool_name {
        "orchestrator_team_create" => Ok("team_create"),
        "orchestrator_team_add_member" => Ok("team_add_member"),
        "orchestrator_team_remove_member" => Ok("team_remove_member"),
        "orchestrator_team_complete" => Ok("team_complete"),
        "orchestrator_team_send_message" => Ok("team_send_message"),
        "orchestrator_team_read_messages" => Ok("team_read_messages"),
        "orchestrator_team_alive_members" => Ok("team_alive_members"),
        "orchestrator_team_list" => Ok("team_list"),
        "orchestrator_agent_register" => Ok("agent_register"),
        "orchestrator_agent_unregister" => Ok("agent_unregister"),
        "orchestrator_agent_list_running" => Ok("agent_list_running"),
        "orchestrator_worker_launch" => Ok("launch"),
        "orchestrator_worker_list" => Ok("list"),
        "orchestrator_worker_terminate" => Ok("terminate"),
        _ => Err(format!("OrchestratorTools: unknown tool: {tool_name}")),
    }
}

/// route_tool: route a natural language query to the best-matching MCP tool.
/// Uses the connection-level host_id by default; args can override with `host_id`.
fn tool_route_tool(
    args: &Value,
    ctx_repo_root: &std::path::Path,
    host_id: &str,
) -> Result<String, String> {
    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or("route_tool: missing 'query' parameter")?;
    let effective_host = args
        .get("host_id")
        .and_then(Value::as_str)
        .filter(|h| !h.is_empty())
        .unwrap_or(host_id);
    let registry_path = resolve_tool_registry_path(ctx_repo_root);
    let decision = tool_routing_engine::routing::route_tool(query, &registry_path, Some(effective_host))?
        .ok_or_else(|| format!("route_tool: no matching tool found for query '{query}'"))?;
    serde_json::to_string(&decision).map_err(|e| e.to_string())
}

/// search_tools: search the tool registry and return top-k results.
/// Uses the connection-level host_id by default; args can override with `host_id`.
fn tool_search_tools(
    args: &Value,
    ctx_repo_root: &std::path::Path,
    host_id: &str,
) -> Result<String, String> {
    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or("search_tools: missing 'query' parameter")?;
    let top_k = args.get("top_k")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let effective_host = args
        .get("host_id")
        .and_then(Value::as_str)
        .filter(|h| !h.is_empty())
        .unwrap_or(host_id);
    let registry_path = resolve_tool_registry_path(ctx_repo_root);
    let records = mcp_tool_registry::load_tool_records_cached(&registry_path)
        .map_err(|e| format!("search_tools: failed to load registry: {e}"))?;
    let results = tool_routing_engine::search::search_tools(query, &records, top_k, Some(effective_host));
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
        "schema_version": mcp_tool_registry::EXPECTED_SCHEMA,
        "total_count": total,
        "builtin_count": builtin,
        "research_count": research,
        "independent_count": independent,
        "external_count": external,
        "registry_path": registry_path,
    });
    serde_json::to_string(&status).map_err(|e| e.to_string())
}

