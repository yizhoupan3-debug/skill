---
last_verified: "2026-06-12"
depends_on:
  - ../host_adapter_contract.md
---

# Opencode 宿主操作手册

**闭集 id**: `opencode` · **传输**: opencode-plugin（JS/TS 插件 hook + MCP 双通道） · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.opencode`

## 代理身份与画风

- 主代理定位为严谨的科研学者与系统工程专家
- 回复画风专业、客观、谦逊；默认使用简体中文
- 不确定的信息直接说明，不编造

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: 插件 hook + MCP 工具层双通道。`tool.execute.before` 可 throw 阻断工具执行（等价 PreToolUse）；`session.idle` 等价 Stop；closeout/review 在 MCP 工具层实现

## 插件 Hook 系统

OpenCode 通过 JS/TS 插件系统提供完整 hook 生命周期：

| OpenCode Hook | 等价于 | 能力 |
|---|---|---|
| `tool.execute.before` | PreToolUse | 拦截工具调用，可修改参数或 throw 阻断 |
| `tool.execute.after` | PostToolUse | 工具执行后处理 |
| `session.idle` | Stop | 会话空闲时触发 |
| `permission.asked` / `permission.replied` | Permission hooks | 权限拦截 |
| `shell.env` | 环境注入 | 注入 shell 环境变量 |

插件加载顺序：全局配置 → 项目配置 → `~/.config/opencode/plugins/` → `.opencode/plugins/`

## opencode.json 配置结构

- 项目级: `./opencode.json`；用户级: `~/.config/opencode/opencode.json`
- MCP 注册字段: `mcp`；Agent 注册字段: `agents`
- 目录自动发现: `.opencode/agents/*.md`、`.opencode/commands/*.md`

## 自定义 Home 目录

- `OPENCODE_HOME` 环境变量可覆盖默认 `~/.opencode` 路径
- `--opencode-home` CLI 标志优先级最高（> `--home` 共享参数 > `OPENCODE_HOME` > 默认值）
- 仅影响框架投影定位，不影响 opencode 自身运行时配置

## 权限与安全模型

- 三类: Allow / Ask / Deny；权限分类: read, write, run, browser
- 框架 MCP server 通常注册为 project scope 的 `mcp`
- `permission.asked` / `permission.replied` 插件 hook 可拦截权限请求

## 自定义 Agent 管理

- 通过 `.opencode/agents/*.md` 文件自动发现
- 通过 `opencode.json` 的 `agents` 字段显式声明
- Harness 投影通过 `.opencode/` 目录注入框架 agent

## 默认生命周期

- `/discussx` → `/planx` → `/implementx` → `/verifyx`
- 默认 `my-light`（advisory closeout）

## 自检诊断

```bash
router-rs framework doctor
router-rs framework host-integration status
```

## Python 环境

- 使用 uv-only、默认 3.12；仓库级 `uv.lock`
