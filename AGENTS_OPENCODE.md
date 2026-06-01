# Opencode Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 opencode 宿主差异。

## 权威分层

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议 | 仓库根 [`AGENTS.md`](AGENTS.md) |
| opencode 专属执行面 | **`AGENTS_OPENCODE.md`**（本文件） |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| MCP 行为 | `opencode.json` 的 `mcpServers` |
| opencode 配置 | `opencode.json` + `~/.config/opencode/opencode.json` |

**文档地图**：[`docs/harness_architecture.md`](docs/harness_architecture.md) · [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md)

## Language

- 跨宿主语言规范见 [`AGENTS.md`](AGENTS.md) § Language；opencode 宿主强制继承，不得豁免。

## Root

- Opencode：项目 `opencode.json` + `~/.config/opencode/opencode.json`；仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。

## 联网入口

- OpenCode 无原生联网工具；如需外网调研，通过 MCP 注册 `browser-mcp`。
- 具体联网能力取决于宿主 provider（Anthropic / OpenAI / Google 等）的 tool-use 支持。

## 子代理（Task 工具）

- Opencode 提供 `Task` 工具用于并行子代理；继承 Codex 的多代理原则：`WAVE_STATE.json` 中 `execution_mode=parallel` 时应主动 spawn，主线程任 scheduler。
- 子代理输出收拢在 `artifacts/current/<task_id>/lane-notes/<lane_id>.md`。

## Harness 传输模式

- **transport: opencode-native**（同 Claude Desktop 的 mcp-stdio 类，无 shell hook）。
- 框架门控：通过 `opencode.json` 的 `mcpServers` 注册 `router-rs-framework`。
- Agent 注入：`opencode.json` 的 `agents` 字段 + `.opencode/` 投影文件。
- Watch 模式：`opencode --watch` 可监听文件变更自动触发；注意与 `artifacts/current/` 的交互边界。

## 默认生命周期

- My 链：`/discussx` → `/planx` → `/implementx` → `/verifyx`
- 默认 `my-light`（关闭 REVIEW_GATE 硬拦）
- 续跑：`framework_goal_drive` stdio + `artifacts/current/<task_id>/` 手动画板

## Knowledge Hygiene

- 本文件是 opencode 地图；跨宿主正文在 [`AGENTS.md`](AGENTS.md)。
