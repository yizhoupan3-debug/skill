# MCP 配置说明

从模板创建配置：

```bash
cp .claude/mcp.json.example .claude/mcp.json
```

然后替换占位符：

- `${REPO_ROOT}` → 仓库根目录的绝对路径
- `${ROUTER_RS_BIN}` → router-rs 二进制的绝对路径（通常在 `~/.local/share/skill-framework/bin/`）
