# mcp.json 路径说明

Claude Desktop MCP 加载器**不支持环境变量插值**（`${VAR}` 语法），且 JSON 格式不支持注释。因此 `mcp.json` 中的路径均为硬编码绝对路径，首次克隆或在新机器上使用时需要手动修改。

## 路径清单

| 路径 | 出现位置 | 含义 | 需修改 |
|------|---------|------|--------|
| `/Users/joe/Developer/skill` | `browser-mcp.args[3]` (`--repo-root`) | 项目仓库根目录 | 是：改为你的本地仓库路径 |
| `/Users/joe/Developer/skill` | `router-rs-framework.args[3]` (`--repo-root`) | 项目仓库根目录 | 是：改为你的本地仓库路径 |
| `/Users/joe/Developer/skill` | `browser-mcp.env.SKILL_FRAMEWORK_ROOT` | 框架根目录（与仓库根相同） | 是：改为你的本地仓库路径 |
| `/Users/joe/Developer/skill` | `router-rs-framework.env.SKILL_FRAMEWORK_ROOT` | 框架根目录（与仓库根相同） | 是：改为你的本地仓库路径 |
| `/Users/joe/.local/share/skill-framework/bin/router-rs` | `browser-mcp.command` | router-rs 二进制文件路径 | 是：改为你的本地安装路径 |
| `/Users/joe/.local/share/skill-framework/bin/router-rs` | `router-rs-framework.command` | router-rs 二进制文件路径 | 是：改为你的本地安装路径 |

所有路径均指向本机目录，不含任何认证令牌或密钥。

## 需修改的路径（占位符形式）

假设你的仓库克隆到 `$REPO_ROOT`，router-rs 二进制安装到 `$ROUTER_RS_BIN`：

```
REPO_ROOT="/your/path/to/skill"
ROUTER_RS_BIN="/your/path/to/router-rs"
```

需要替换的 6 处：

1. `mcpServers.browser-mcp.args[3]` -- 改为 `$REPO_ROOT`
2. `mcpServers.browser-mcp.env.SKILL_FRAMEWORK_ROOT` -- 改为 `$REPO_ROOT`
3. `mcpServers.browser-mcp.command` -- 改为 `$ROUTER_RS_BIN`
4. `mcpServers.router-rs-framework.args[3]` -- 改为 `$REPO_ROOT`
5. `mcpServers.router-rs-framework.env.SKILL_FRAMEWORK_ROOT` -- 改为 `$REPO_ROOT`
6. `mcpServers.router-rs-framework.command` -- 改为 `$ROUTER_RS_BIN`

## 首次设置步骤

1. 克隆仓库到本地：
   ```bash
   git clone <repo-url> /your/path/to/skill
   ```

2. 安装 router-rs 二进制（参见框架安装文档）。

3. 复制 `mcp.json` 并替换路径：
   ```bash
   cp .claude/mcp.json .claude/mcp.json.bak
   # 用编辑器打开 .claude/mcp.json，将所有 /Users/joe/Developer/skill
   # 替换为你的仓库实际路径
   # 将 /Users/joe/.local/share/skill-framework/bin/router-rs
   # 替换为你的 router-rs 二进制实际路径
   ```

4. 重启 Claude Desktop，确认 MCP 工具栏出现 `browser-mcp` 和 `router-rs-framework`。

## MCP 服务器说明

| 服务器 | 用途 |
|--------|------|
| `browser-mcp` | 外网调研：`browser_open`、`browser_get_text`、`browser_click` 等浏览器自动化工具 |
| `router-rs-framework` | 框架运行时：`framework_snapshot`、`skill_route`、`goal_state_manage`、`closeout_gate` 等生命周期管理工具 |

## 版本说明

- 本文档适用于 Claude Desktop MCP 加载器，该加载器以 JSON 原文解析 `mcp.json`，不做变量展开。
- 如果未来 Claude Desktop 支持 `${VAR}` 插值，可将硬编码路径替换为环境变量引用，届时本文档可简化。
- `mcp.json` 位于 `.claude/` 目录下，属于 Claude Desktop 项目级配置（非全局配置）。
