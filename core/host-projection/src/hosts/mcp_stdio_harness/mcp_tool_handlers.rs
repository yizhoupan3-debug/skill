use super::*;
use core_errors::FrameworkError;
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
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError>;
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
                tracing::error!(
                    "CompositeRegistry: duplicate tool name '{name}' \
                     registered — existing handler WILL be overwritten. \
                     Fix tool registration to avoid silent dispatch divergence."
                );
            }
            self.name_to_handler.insert(name, idx);
        }
        self.handlers.push(Box::new(handler));
    }

    pub fn contains(&self, tool_name: &str) -> bool {
        self.name_to_handler.contains_key(tool_name)
    }

    pub fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        if let Some(&idx) = self.name_to_handler.get(tool_name) {
            self.handlers[idx].dispatch(tool_name, args, ctx)
        } else {
            Err(FrameworkError::not_found(format!(
                "Unknown tool: {tool_name}"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// RoutingTools (4 tools)
// ---------------------------------------------------------------------------

pub struct RoutingTools;
impl ToolHandler for RoutingTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "skill_route",
            "skill_search",
            "skill_read",
            "skill_route_status",
        ]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "skill_route" => tool_skill_route(args, &ctx.repo_root, &ctx.host_id),
            "skill_search" => tool_skill_search(args, &ctx.repo_root, &ctx.host_id),
            "skill_read" => tool_skill_read(args, &ctx.repo_root),
            "skill_route_status" => tool_skill_route_status(&ctx.repo_root),
            _ => Err(FrameworkError::not_found(format!(
                "RoutingTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
pub struct ToolDomainTools;
impl ToolHandler for ToolDomainTools {
    fn tool_names(&self) -> &[&'static str] {
        &["search_tools", "tool_registry_status"]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "search_tools" => tool_search_tools(args, &ctx.repo_root, &ctx.host_id),
            "tool_registry_status" => tool_registry_status(),
            _ => Err(FrameworkError::not_found(format!(
                "ToolDomainTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskCrudTools (4 tools: task_create, task_list, task_complete, task_focus)
// ---------------------------------------------------------------------------

pub struct TaskCrudTools;
impl ToolHandler for TaskCrudTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "task_create",
            "task_list",
            "task_complete",
            "task_focus",
            "task_chain_advance",
        ]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "task_create" => Ok(tool_task_create(args, &ctx.repo_root)?),
            "task_list" => Ok(tool_task_list(&ctx.repo_root)?),
            "task_complete" => Ok(tool_task_complete(args, &ctx.repo_root)?),
            "task_focus" => Ok(tool_task_focus(args, &ctx.repo_root)?),
            "task_chain_advance" => Ok(tool_task_chain_advance(args, &ctx.repo_root)?),
            _ => Err(FrameworkError::not_found(format!(
                "TaskCrudTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// LoopControlTools (3 tools: loop_pause, loop_resume, loop_redirect)
// Write goal-engine signal files via task_tools handlers (direct fs, no dep).
// ---------------------------------------------------------------------------

pub struct LoopControlTools;
impl ToolHandler for LoopControlTools {
    fn tool_names(&self) -> &[&'static str] {
        &["loop_pause", "loop_resume", "loop_redirect"]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "loop_pause" => Ok(tool_loop_pause(args, &ctx.repo_root)?),
            "loop_resume" => Ok(tool_loop_resume(args, &ctx.repo_root)?),
            "loop_redirect" => Ok(tool_loop_redirect(args, &ctx.repo_root)?),
            _ => Err(FrameworkError::not_found(format!(
                "LoopControlTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// GoalCloseoutTools (4 tools: closeout_gate, closeout_record_write, goal_state_manage, goal_state_read)
// ---------------------------------------------------------------------------

pub struct GoalCloseoutTools;
impl ToolHandler for GoalCloseoutTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "closeout_gate",
            "closeout_record_write",
            "goal_state_manage",
            "goal_state_read",
            "record_evidence",
            "session_checkpoint",
        ]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "closeout_gate" => tool_closeout_gate(args, &ctx.repo_root, &ctx.host_id),
            "closeout_record_write" => tool_closeout_record_write(args, &ctx.repo_root, &ctx.host_id),
            "goal_state_manage" => tool_goal_state_manage(args, &ctx.repo_root, &ctx.connection_session_id),
            "goal_state_read" => tool_goal_state_read(args, &ctx.repo_root),
            "record_evidence" => tool_record_evidence(args, &ctx.repo_root),
            "session_checkpoint" => tool_session_checkpoint(&ctx.repo_root),
            _ => Err(FrameworkError::not_found(format!(
                "GoalCloseoutTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskOutputTools (6 tools: task_output_write, task_output_read, task_output_init,
//                   task_output_pull, task_output_validate, chain_aggregate)
// ---------------------------------------------------------------------------

pub struct TaskOutputTools;
impl ToolHandler for TaskOutputTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "task_output_write",
            "task_output_read",
            "task_output_init",
            "task_output_pull",
            "task_output_validate",
            "chain_aggregate",
        ]
    }
    fn dispatch(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ToolCallContext,
    ) -> Result<String, FrameworkError> {
        match tool_name {
            "task_output_write" => Ok(tool_task_output_write(args, &ctx.repo_root)?),
            "task_output_read" => Ok(tool_task_output_read(args, &ctx.repo_root)?),
            "task_output_init" => Ok(tool_task_output_init(args, &ctx.repo_root)?),
            "task_output_pull" => Ok(tool_task_output_pull(args, &ctx.repo_root)?),
            "task_output_validate" => Ok(tool_task_output_validate(args, &ctx.repo_root)?),
            "chain_aggregate" => Ok(tool_chain_aggregate(args, &ctx.repo_root)?),
            _ => Err(FrameworkError::not_found(format!(
                "TaskOutputTools: unknown tool: {tool_name}"
            ))),
        }
    }
}

/// Resolve the tool registry path. Uses hooks, falls back to repo_root default.
fn resolve_tool_registry_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    mcp_tool_registry::resolve_tool_registry_path().unwrap_or_else(|| {
        repo_root.join(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH)
    })
}

// ---------------------------------------------------------------------------

/// search_tools: search the tool registry and return top-k results.
/// Uses the connection-level host_id by default; args can override with `host_id`.
fn tool_search_tools(
    args: &Value,
    ctx_repo_root: &std::path::Path,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
        FrameworkError::from("search_tools: missing 'query' parameter".to_string())
    })?;
    let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    // P2 #03: reject caller-supplied host_id override; always use connection-level identity.
    if let Some(override_host) = args.get("host_id").and_then(Value::as_str).filter(|h| !h.is_empty()) {
        if override_host != host_id {
            tracing::warn!("host_id override rejected: caller tried to override '{host_id}' with '{override_host}'");
        }
    }
    let registry_path = resolve_tool_registry_path(ctx_repo_root);
    let records = mcp_tool_registry::load_tool_records_cached(&registry_path)
        .map_err(|e| FrameworkError::from(format!("search_tools: failed to load registry: {e}")))?;
    let results =
        tool_routing_engine::search::search_tools(query, &records, top_k);
    serde_json::to_string(&results).map_err(|e| FrameworkError::from(e.to_string()))
}

/// tool_registry_status: report registry metadata (count, schema version, layers).
fn tool_registry_status() -> Result<String, FrameworkError> {
    let registry_path = mcp_tool_registry::resolve_tool_registry_path().ok_or_else(|| {
        FrameworkError::from(
            "tool_registry_status: registry path not configured (hooks not registered)".to_string(),
        )
    })?;
    let records = mcp_tool_registry::load_tool_records_cached(&registry_path).map_err(|e| {
        FrameworkError::from(format!(
            "tool_registry_status: failed to load registry: {e}"
        ))
    })?;
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
    serde_json::to_string(&status).map_err(|e| FrameworkError::from(e.to_string()))
}
