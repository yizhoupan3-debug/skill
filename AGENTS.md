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
