use serde_json::Value;

/// 统一工具注册表接口。
pub trait ToolRegistry: Send + Sync {
    /// 返回此注册表管理的所有工具定义（用于 MCP tools/list 响应）。
    fn tool_definitions(&self) -> Vec<Value>;

    /// 按工具名查找工具定义。
    fn find_tool(&self, name: &str) -> Option<&Value>;

    /// 注册一个工具定义。
    fn register_tool(&mut self, tool: Value);
}

/// 默认的 MCP 工具注册表实现。
#[derive(Default)]
pub struct McpToolRegistry {
    tools: Vec<Value>,
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }
}

impl ToolRegistry for McpToolRegistry {
    fn tool_definitions(&self) -> Vec<Value> {
        self.tools.clone()
    }

    fn find_tool(&self, name: &str) -> Option<&Value> {
        self.tools.iter().find(|t| {
            t.get("name").and_then(Value::as_str) == Some(name)
        })
    }

    fn register_tool(&mut self, tool: Value) {
        self.tools.push(tool);
    }
}
