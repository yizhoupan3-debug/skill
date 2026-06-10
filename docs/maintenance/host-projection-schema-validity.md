---
last_verified: "2026-06-04"
scope: 非 Claude 宿主的 host projection schema 校验
depends_on:
  - ../host_adapter_contract.md
  - ../framework_naming_conventions.md
  - ../maintenance/ops-runbook.md
  - ../../configs/framework/RUNTIME_REGISTRY.json
related_incidents:
  - "2026-06-04 opencode desktop 报「无法重新加载 skill」/ConfigInvalidError 根因（投影写 `mcpServers` 而非 `mcp`）"
---

# Host Projection Schema Validity Reference

> **目的**：防止 projection 生成的配置键名/路径与宿主真实 schema 不一致。
>
> 2026-06-04 发现 opencode/cursor/antigravity 三宿主同款 bug：projection adapter 从一个宿主抄到另一个时 key 名未按目标宿主真实 schema 改。**本文仅保留闭集矩阵与自检机制；详细 bug 清单与修复 PR 顺序见源码 `projection.rs` 及 git history。**

## 闭集 MCP Key 矩阵（新增宿主须先填这一行）

> 每一行的键名/路径必须是该宿主官方文档/源码实测值，不得从一个宿主"借鉴"。

| host_id | 实际配置文件（user scope） | MCP 顶层 key | transport | 必填字段 | `managed_key_paths` |
|---------|---------------------------|-------------|-----------|----------|-------------------|
| `claude-code` | 项目 `.mcp.json` | `mcpServers` (camel) | `"stdio"` | `command` | `mcpServers.router-rs-framework` · `mcpServers.browser-mcp` · `mcpServers.mcp-codegraph` · `mcpServers.paperplain` |
| `codex` | `.codex/config.toml` | `[mcp_servers]` (TOML snake) | 隐式 | `command` | `mcp_servers.router-rs-framework` · `mcp_servers.browser-mcp` · `mcp_servers.mcp-codegraph` · `mcp_servers.paperplain` |
| `cursor` | `~/.cursor/mcp.json` | `mcp_servers` (**snake_case JSON**) | `"stdio"` | `command` | `mcp_servers.router-rs-framework` · `mcp_servers.browser-mcp` · `mcp_servers.mcp-codegraph` · `mcp_servers.paperplain` |
| `opencode` | `~/.config/opencode/opencode.json` | `mcpServers` (camel) | `"local"` | `command: string[]` | `mcpServers.router-rs-framework` · `mcpServers.browser-mcp` · `mcpServers.mcp-codegraph` · `mcpServers.paperplain` |
| `antigravity` | `~/.gemini/antigravity/mcp.json` | `mcpServers` | `"stdio"` | `command` | `mcpServers.router-rs-framework` · `mcpServers.browser-mcp` · `mcpServers.mcp-codegraph` · `mcpServers.paperplain` |

**三个易踩坑**：① TOML snake_case vs JSON camelCase；② Cursor 用 `mcp_servers`（snake），其余 JSON 宿主用 `mcpServers`（camel）；③ opencode 唯一用 `"local"` 而非 `"stdio"`。

## 自检机制（写盘前/后强制校验）

所有 `install_<host>_projection` 须实现：
1. **写盘前**：`validate_projection_payload_against_host_schema(host_id, payload)` — 校验顶层 key + transport 字面量
2. **写盘后**：`readback_validate_<host>(path)` — 反读文件断言 key 名一致
3. **路径存在性**：manifest 中每条 `managed_key_paths` 能在 payload 中定位到非空 entry
4. **测试夹具与生产同源**：共享 `make_<host>_payload()` 工厂函数，禁止 fixture 锁死 bug 形态

## 历史根因

2026-06-04 opencode `mcpServers` → `mcp`（多 `Servers` 后缀）；cursor `mcp_servers` → `mcpServers`（少大写 S）；antigravity `mcp.json` → `mcp_config.json`（错文件路径）。**共同模式**：跨宿主模板未适配目标宿主实际 schema。**自检机制是唯一长期解。**
