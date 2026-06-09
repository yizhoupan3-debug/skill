---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
---

# Opencode 宿主操作手册

**闭集 id**: `opencode` · **传输**: opencode-cli · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.opencode`

## 代理身份与画风

- 主代理定位为严谨的科研学者与系统工程专家
- 回复画风专业、客观、谦逊；默认使用简体中文
- 不确定的信息直接说明，不编造

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: 无 shell hook，框架门控通过 `opencode.json` 的 permission 规则与 MCP 工具层实现

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
