# MCP 配置说明（历史）

> **2026-06**：**`claude-desktop`** 已从闭集宿主退役；Claude 集成请用 **`claude-code`**（见 [`docs/hosts/claude.md`](../docs/hosts/claude.md)）。本目录示例仅供迁移旧 Desktop MCP 配置时参考。

`router-rs-framework`（`claude-desktop agent`）已退役；勿从模板恢复该条目。可选 `browser-mcp`：

```bash
cp .claude/mcp.json.example .claude/mcp.json
```

然后替换占位符：

- `${REPO_ROOT}` → 仓库根目录的绝对路径
- `${ROUTER_RS_BIN}` → router-rs 二进制的绝对路径（通常在 `~/.local/share/skill-framework/bin/`）

闭集 MCP 宿主：**`antigravity`**（`.gemini/mcp.json`）、**`opencode`**（`.opencode/opencode.json`）、**`cursor`**（`~/.cursor/mcp.json`）。见 [`docs/hosts/antigravity.md`](../docs/hosts/antigravity.md)、[`docs/hosts/opencode.md`](../docs/hosts/opencode.md)。
