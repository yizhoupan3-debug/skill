---
name: mcp-server-management
description: |
  创建、配置、调试和注册 MCP server，实现 AI agent 工具集成。
  覆盖 stdio/SSE/streamable-http 三种传输模式，Python FastMCP 和 Rust 两种主流实现，
  以及宿主集成（MCP client 配置）、MCP Registry 注册、调试故障排查全流程。
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [mcp, mcp-server, tool-integration, fastmcp, model-context-protocol]
risk: low
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: never
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - MCP server
  - 创建 MCP
  - MCP 调试
  - MCP 注册
  - MCP 配置
  - tool server
  - MCP 开发
  - model context protocol
---

## 概述

MCP (Model Context Protocol) 是 AI agent 连接外部工具和数据的事实标准协议。
它定义了 host（宿主环境）、client 和 server 之间的通信规范，
使 agent 能够调用外部工具、读取资源、使用 prompt 模板。

核心概念：
- **Tool**：server 暴露的可调用函数（`@tool` 装饰器 / `list_tools` + `call_tool`）
- **Resource**：server 暴露的只读数据源（`@resource` 装饰器 / `list_resources` + `read_resource`）
- **Prompt**：server 提供的 prompt 模板（`@prompt` 装饰器 / `list_prompts` + `get_prompt`）

## MCP Server 创建

### 传输模式

| 模式 | 适用场景 | 特点 |
|------|----------|------|
| **stdio** | 本地进程、CLI 工具 | 最简单，host 直接 spawn 子进程 |
| **SSE** | 远程服务、Web 部署 | HTTP 长连接，Server-Sent Events |
| **streamable-http** | 远程服务（推荐） | MCP 2025-03-26 新增，取代 SSE |

### Python：FastMCP 框架

```python
from fastmcp import FastMCP

mcp = FastMCP("my-server")

@mcp.tool()
def add(a: int, b: int) -> int:
    """两数相加"""
    return a + b

@mcp.resource("greeting://{name}")
def greeting(name: str) -> str:
    return f"Hello, {name}!"

@mcp.prompt()
def review_prompt(code: str) -> str:
    return f"请 review 以下代码:\n{code}"

if __name__ == "__main__":
    mcp.run()  # stdio 模式
    # mcp.run(transport="sse", port=8000)
    # mcp.run(transport="streamable-http", port=8000)
```

安装：`pip install fastmcp` 或 `uv add fastmcp`

### Rust 实现

参考本仓库四个 Rust MCP server（跨四宿主统一注册）：
- `router-rs-framework`：stdio 模式，框架路由 / goal / closeout 工具集
- `browser-mcp`：stdio 模式，带 session supervisor 的浏览器自动化
- `mcp-codegraph`：stdio 模式，代码知识图谱（search/callers/callees/impact）
- `paperplain`：stdio 模式，学术论文元数据检索（`npx -y paperplain-mcp`）

Rust MCP server 通常基于 `rmcp` crate 或自行实现 JSON-RPC over stdio。

### 最小可运行示例（Python stdio）

```python
# server.py
from fastmcp import FastMCP

mcp = FastMCP("demo")

@mcp.tool()
def hello(name: str) -> str:
    """Say hello"""
    return f"Hello, {name}!"

mcp.run()
```

验证：`echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}' | python server.py`

## 集成到宿主环境

### .mcp.json（项目级）

在项目根目录创建 `.mcp.json`：

```json
{
  "mcpServers": {
    "my-server": {
      "command": "python",
      "args": ["server.py"],
      "env": { "API_KEY": "xxx" }
    }
  }
}
```

### settings.json（用户级 / 项目级）

```json
{
  "mcpServers": {
    "my-server": {
      "command": "uv",
      "args": ["run", "--directory", "/path/to/server", "server.py"],
      "type": "stdio"
    },
    "remote-server": {
      "type": "streamable-http",
      "url": "https://example.com/mcp"
    }
  }
}
```

### 权限管理

- `allowedTools`：白名单工具列表（格式 `mcp__<server>__<tool>`）
- `trust` level：控制 MCP server 的执行权限
- 安全原则：MCP server 与 host 同权限，不可提升

## MCP Registry 注册

### 流程

1. 确保 server 实现符合 MCP 规范（`modelcontextprotocol.io`）
2. 准备包元数据：名称、描述、版本、传输模式、依赖
3. 发布到 npm / PyPI / crates.io
4. 提交到 `registry.modelcontextprotocol.io`：
   - Fork `modelcontextprotocol/registry` 仓库
   - 添加 JSON 描述文件
   - 提交 PR

### 元数据规范

```json
{
  "name": "my-mcp-server",
  "description": "简短描述",
  "version": "1.0.0",
  "repository": "https://github.com/...",
  "packages": [
    {
      "registry": "npm",
      "name": "my-mcp-server",
      "version": "1.0.0"
    }
  ]
}
```

## 调试与故障排查

### MCP Inspector

官方调试工具，可视化 MCP 通信：

```bash
npx @modelcontextprotocol/inspector python server.py
```

打开 `http://localhost:5173` 可交互式测试 tool/resource/prompt。

### 日志协议

MCP 定义了 `logging/setLevel` 和 `notifications/message` 方法。
Server 可通过 `ctx.info()` / `ctx.warning()` / `ctx.error()` 发送日志到 client。

### 常见问题

| 问题 | 原因 | 解决 |
|------|------|------|
| 连接超时 | server 启动慢或 crash | 检查 server 进程、增大 timeout |
| 权限拒绝 | `allowedTools` 未包含 | 添加工具到白名单 |
| Schema 不匹配 | tool 参数类型不符 | 检查 JSON Schema 定义与实际入参 |
| Server 无响应 | stdio 模式下 stdout 被污染 | 确保 server 不 print 到 stdout |
| 初始化失败 | protocolVersion 不兼容 | 升级 MCP SDK 到最新版 |

## 当前仓库参考

- **router-rs-framework**：`core/router-rs/` — Rust stdio MCP server，提供框架路由、goal 管理、closeout 等工具
- **browser-mcp**：`core/browser-mcp/` — Rust stdio MCP（`router-rs browser mcp-stdio`），浏览器自动化，带 session supervisor 和 background job 管理
- **mcp-codegraph**：`tools/` 独立 crate `codegraph-rs` — Rust stdio MCP，代码知识图谱（search/callers/callees/impact/node/status）
- **paperplain**：`npx -y paperplain-mcp` — Node stdio MCP，学术论文元数据检索（paper_metadata/paper_search）

## Exit Criteria

- MCP 服务器可启动（进程存活 + 无 crash）
- 工具可调用（至少一个 tool 返回正常响应）
- 配置已持久化到 settings.json
