//! Tool vs Skill namespace isolation — tool origin classification.
//!
//! Extracted from `hook_common.rs` (2026-06-25 refactor) to reduce file
//! size and clarify responsibility boundaries within `core-policy`.

use serde_json::{Map, Value};

// ────────────────────────────────────────────────────────────────
// Tool vs Skill namespace isolation
// ────────────────────────────────────────────────────────────────

/// 工具来源分类，用于隔离 hook 事件处理中的 tool vs skill 边界。
///
/// - `NativeHost`：宿主内置工具（Bash, Write, Edit, Read, Agent 等）
/// - `McpServer`：MCP 工具，FQN 格式 `mcp__{server_id}__{tool_name}`
/// - `Unknown`：未识别的工具名（可能是新宿主工具或第三方扩展）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOrigin {
    NativeHost,
    McpServer {
        server_id: String,
        tool_name: String,
    },
    Unknown,
}

impl ToolOrigin {
    /// 是否为 MCP 工具
    pub fn is_mcp(&self) -> bool {
        matches!(self, ToolOrigin::McpServer { .. })
    }

    /// 是否为宿主内置工具
    pub fn is_native(&self) -> bool {
        matches!(self, ToolOrigin::NativeHost)
    }
}

/// 判断工具名是否为 MCP 工具 FQN（`mcp__{server}__{tool}`）。
pub fn is_mcp_tool_name(name: &str) -> bool {
    name.starts_with("mcp__")
}

/// 解析 MCP 工具 FQN：`mcp__{server_id}__{tool_name}`。
///
/// 使用 `rsplit_once("__")` 从右侧解析，支持 server_id 包含连字符
/// （如 `browser-mcp`、`router-rs-framework`）。
pub fn parse_mcp_tool_fqn(fqn: &str) -> Option<(&str, &str)> {
    let rest = fqn.strip_prefix("mcp__")?;
    let (server_id, tool_name) = rest.rsplit_once("__")?;
    if server_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some((server_id, tool_name))
}

/// 已知宿主内置工具闭集（跨 Claude/Cursor/Codex/OpenCode 全覆盖）。
///
/// 包含：宿主原生工具 + 跨宿主 shell/写入/子代理工具。
fn is_known_native_tool(name: &str) -> bool {
    matches!(
        name,
        // Claude 原生工具
        "Bash" | "Write" | "Edit" | "Read" | "Agent" | "NotebookEdit"
        | "WebSearch" | "WebFetch" | "Glob" | "Grep" | "LS"
        | "SendMessage" | "Skill" | "EnterWorktree" | "ExitWorktree"
        | "TeamCreate" | "DesignSync" | "CronCreate" | "CronDelete" | "CronList"
        // 跨宿主 shell 工具（is_shell_tool 闭集）
        | "shell" | "bash" | "run_terminal_cmd" | "execute_command"
        | "terminal" | "run_command" | "sh" | "exec" | "cmd"
        // 跨宿主写入工具（is_file_write_tool 闭集）
        | "write" | "strreplace" | "str_replace" | "delete"
        | "applypatch" | "apply_patch" | "notebookedit" | "notebook_edit"
        // 子代理工具（SUBAGENT_TOOL_NAMES + 扩展）
        | "task" | "subagent" | "spawn_agent" | "dispatch_agent"
        | "functions.task" | "functions.subagent" | "functions.spawn_agent"
        | "functions.exec_command"
    )
}

/// 分类工具来源。
///
/// 优先检查 MCP FQN 格式，再检查宿主内置工具闭集，最后归为 Unknown。
pub fn classify_tool_origin(tool_name: &str) -> ToolOrigin {
    if let Some((server, tool)) = parse_mcp_tool_fqn(tool_name) {
        ToolOrigin::McpServer {
            server_id: server.to_string(),
            tool_name: tool.to_string(),
        }
    } else if is_known_native_tool(tool_name) {
        ToolOrigin::NativeHost
    } else {
        ToolOrigin::Unknown
    }
}

/// Merge hook payloads' tool argument object from common alternate keys (`tool_input`, `input`,
/// `arguments`, `parameters`). Shared by all hosts' nested stdin extraction and tool parsing.
pub fn tool_input_value_from_map(obj: &Map<String, Value>) -> Option<Value> {
    obj.get("tool_input")
        .or_else(|| obj.get("input"))
        .or_else(|| obj.get("arguments"))
        .or_else(|| obj.get("parameters"))
        .cloned()
}

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_lowercase()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mcp_tool_fqn_basic() {
        assert_eq!(
            parse_mcp_tool_fqn("mcp__browser-mcp__browser_click"),
            Some(("browser-mcp", "browser_click"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__router-rs-framework__goal_state_manage"),
            Some(("router-rs-framework", "goal_state_manage"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__paperplain__search_research"),
            Some(("paperplain", "search_research"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__mcp-codegraph__codegraph_search"),
            Some(("mcp-codegraph", "codegraph_search"))
        );
    }

    #[test]
    fn parse_mcp_tool_fqn_rejects_invalid() {
        assert_eq!(parse_mcp_tool_fqn("Bash"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp__"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp__server__"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp____tool"), None);
        assert_eq!(parse_mcp_tool_fqn(""), None);
    }

    #[test]
    fn is_mcp_tool_name_works() {
        assert!(is_mcp_tool_name("mcp__browser-mcp__browser_click"));
        assert!(!is_mcp_tool_name("Bash"));
        assert!(!is_mcp_tool_name(""));
        assert!(!is_mcp_tool_name("mcp_tool")); // single underscore
    }

    #[test]
    fn classify_tool_origin_mcp() {
        let origin = classify_tool_origin("mcp__browser-mcp__browser_click");
        assert!(origin.is_mcp());
        assert!(!origin.is_native());
        match &origin {
            ToolOrigin::McpServer { server_id, tool_name } => {
                assert_eq!(server_id, "browser-mcp");
                assert_eq!(tool_name, "browser_click");
            }
            _ => panic!("expected McpServer"),
        }
    }

    #[test]
    fn classify_tool_origin_native() {
        for tool in &["Bash", "Write", "Edit", "Read", "Agent", "shell", "bash", "task"] {
            let origin = classify_tool_origin(tool);
            assert!(origin.is_native(), "{tool} should be NativeHost");
        }
    }

    #[test]
    fn classify_tool_origin_unknown() {
        let origin = classify_tool_origin("SomeNewTool");
        assert_eq!(origin, ToolOrigin::Unknown);
    }
}
