# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，禁止在 git worktree 中运行或修改任何文件。

## Review

- **Review findings-only by default**: review 产出仅为 findings（P0→P1→P2→Caveat），不默认改代码、不执行。参见 `skills/code-review-deep/SKILL.md`。
- Closeout: 完成时使用 `goal_state_manage(operation=complete)` 记录 closeout evidence。
- Skill Routing: 使用 `skill_route` / `skill_search` MCP 工具进行路由。
- Goal state: 通过 `goal_state_read` / `goal_state_manage` 管理目标状态。
