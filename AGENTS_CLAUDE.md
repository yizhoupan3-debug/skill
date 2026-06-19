# Claude Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **Claude**（`claude`）transport delta；review 清门为跨宿主 **canonical** 参考实现。手册 [`docs/hosts/hook-hosts.md`](docs/hosts/hook-hosts.md) · [`docs/spec.md`](docs/spec.md) §0.1。

## PreToolUse 硬阻断（Claude 独有）

`.claude/settings.json` + `pre-tool-use` hook：未物化 `GOAL_STATE.json` 或未授权执行区 → **硬阻断**（`continue: false`）。遭遇阻断时 `/discussx` 或 `/planx` 自愈，勿盲目重试。`ROUTER_RS_CLAUDE_*` 仅作用于本宿主 hook。

## Transport 要点

- **Review gate（canonical）**：清门语义以 [`host_adapter_contract.md`](docs/spec.md) §0.1 为准；Stop **`REVIEW_GATE` advisory-only**（`router-rs CLAUDE_REVIEW_GATE …`）；**无** `rg_clear` 粘贴面（须完成可数 reviewer lane 或自然语言 override）。
- **框架命令流**：无 `AG_FOLLOWUP` / `updateCurrentStep`；续跑 `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **my-light**：suppress spawn-first 与 review Stop nudge（skill 层 findings-only 仍适用）。

## CodeGraph 自动触发（Claude 执行细则）

**跨宿主规则见 [`AGENTS.md`](AGENTS.md) § CodeGraph 自动触发规则**

Claude 宿主执行要点：
1. **自动识别**：从用户输入中识别触发词（重构、删除、跨模块等），自动调用对应codegraph工具
2. **无需询问**：直接调用工具，不询问用户是否要使用codegraph
3. **结果整合**：将工具结果整合到响应中，说明影响范围和风险
4. **强制执行**：无论是否触发特定技能，都必须执行自动触发规则

**示例场景**：
```
用户：帮我重构这个函数
Claude：（自动调用codegraph_impact分析影响范围）→ 根据结果制定重构计划
```
