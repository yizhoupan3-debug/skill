# Cursor Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。本文仅 Cursor 宿主差异。

## 权威分层（改哪里才生效）

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议 | 仓库根 [`AGENTS.md`](AGENTS.md) |
| Cursor 执行面默认值 | **`AGENTS_CURSOR.md`**（本文件）+ [`.cursor/rules/*-gate.mdc`](.cursor/rules/) |
| Cursor framework 叙事 | `router-rs framework host-integration install --to cursor --scope user` → `~/.cursor/rules/framework.mdc` |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | `.cursor/hooks.json` + `router-rs` |

**文档地图**：[`docs/harness_architecture.md`](docs/harness_architecture.md) · [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) · [`docs/hosts/cursor.md`](docs/hosts/cursor.md)

## Root

- Cursor：`CURSOR_HOME`；仓库内优先 `skills/` 与 `skills/SKILL_ROUTING_RUNTIME.json`。

## Host Boundaries (Cursor 专属硬约束与门控)

- **机读短码**：宿主注入单行 `AG_FOLLOWUP`、`REVIEW_GATE` 等；**严禁自拟、伪造或扩展仿 hook 长文**。续跑须 `/implementx` + `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **`updateCurrentStep`**：**严禁空载荷**；须含可机读的状态、步骤索引或执行描述。
- **子代理模型继承**：并行 `Task` **默认继承主会话模型**（省略 `model`）；禁止默认 claude/sonnet，除非主会话已用 Anthropic。见 [`.cursor/rules/subagent-model-inherit.mdc`](.cursor/rules/subagent-model-inherit.mdc)、[`docs/hosts/cursor.md`](docs/hosts/cursor.md)。
- 路由问题 → runtime；hook 问题 → `.cursor/hooks.json`。

## Execution Ladder（Cursor 差异）

- 遵从 [`.cursor/rules/execution-subagent-gate.mdc`](.cursor/rules/execution-subagent-gate.mdc)、[`.cursor/rules/review-subagent-gate.mdc`](.cursor/rules/review-subagent-gate.mdc)（lane / hook / `updateCurrentStep` 硬约束）。
- 完整梯子：[`docs/references/EXECUTION_LADDER.md`](docs/references/EXECUTION_LADDER.md)；REVIEW_GATE 操作：[`docs/framework_operator_primer.md`](docs/framework_operator_primer.md)。

## Knowledge Hygiene

- 本文件是 Cursor 地图；跨宿主正文在 [`AGENTS.md`](AGENTS.md)。
