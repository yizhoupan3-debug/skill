# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- 输出规则见用户级 `~/.claude/CLAUDE.md` OUTPUT RULES 节（始终生效，全项目覆盖）。

## Coding First Principles

## Coding First Principles

- Five gates: Goal/Non-goals/Owner/Minimal delta/Validation.
- Evidence closeout (test/diff/blocker). No future-proof abstraction.

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，禁止在 git worktree 中运行或修改任何文件。

## Review

- Findings-only by default (P0→P1→P2→Caveat). No auto-fix, no auto-exec.
- Closeout via `goal_state_manage(operation=complete)`.

## 路由规则

| 输入 | 路由 |
|------|------|
| `/cmd` slash command | `~/.claude/skills/<name>/SKILL.md`（Claude 原生，不经过 MCP） |
| NL 任务 | `skill_route(query)` → `recommended_tools` + `selected_skill` |
| 工具探索 | `search_tools(query, top_k)` |

**新增 skill checklist**: 创建 `~/.claude/skills/<name>/SKILL.md` → 确认 `RUNTIME_REGISTRY.json` + `SKILL_ROUTING_RUNTIME.json` 无冲突。
工具注册表见 `configs/framework/MCP_TOOL_REGISTRY.json`。

---

## Harness 速查卡

| 场景 | 行动 |
|------|------|
| 科研入口/文献/审稿/实验 | `$research` |
| 选题尖锐化 | `$good-question` |
| 故事线诊断/结果→叙事 | `$good-story` |
| 深度网络搜索 + 事实核查 | `$deep-search` |
| 数学推导/求解/积分/极限/级数 | `math_*` 对应 MCP 工具 |
| 约束系统/优化/证明 | `math_z3_*` |
| 论文/文档/PDF/PPT/Excel | `$paper-workbench` / `$doc` / `$pdf` / `$slides` / `$spreadsheets` |
| 代码审查（只审不改） | `$code-review-deep` |
| 代码简化（复用+质量） | `$simplify` |
| 部件烟雾测试（内部诊断+外部评估） | `$smoke` |
| 根因未知故障排查 | `$systematic-debugging` |
| CI 失败修复 / PR 评论回复 | `$gh-fix-ci` / `$gh-address-comments` |
| Goal/Task 生命周期 | `goal_state_manage(...)` / `task_create` / `task_focus` / `task_complete` |
| DAG 编排 | `chain_dag_init` / `chain_dag_tick` |
| 文献/统计/实验验证 | `research_verification_*` 系列 |
| 框架运维/路由治理 | `$skill-framework-developer` |
| 设计/原型/品牌 | `$design-md` / `$visual-review` / `$hallmark` / `$huashu-design` |
| MCP server 管理 | `$mcp-server-management` |
| 出版级科研图表 | `$scipilot-figure` / `$tikz-paper-figure` |

---

## Skill 目录

`skill_route(query)` 路由。顶刊专版 + `$ajs-*` 通配符扩展。

`$research` · `$good-question` · `$good-story` · `$smoke` · `$deep-search` · `$paper-workbench` · `$citation-management` · `$experiment-reproducibility` · `$statistical-analysis` · `$financial-data` · `$math-verify` · `$math-explore` · `$math-modeling` · `$ajs-*` (9本顶刊) · `$code-review-deep` · `$simplify` · `$systematic-debugging` · `$sentry` · `$gh-fix-ci` · `$gitx` · `$gh-address-comments` · `$update` · `$doc` · `$pdf` · `$slides` · `$spreadsheets` · `$design-md` · `$visual-review` · `$tikz-paper-figure` · `$scipilot-figure` · `$huashu-design` · `$hallmark` · `$research-workspace` · `$web-tools` · `$mcp-server-management` · `$python-env-management` · `$plan-mode` · `$deepinterview` · `$goalx` · `$initx` · `$skill-framework-developer` · `$primary-runtime`

## 参考文档

- **Task Output / Chain DAG** 详见 [`AGENTS_REFERENCE.md`](AGENTS_REFERENCE.md)
- **路由分层详解**见 `skills/SKILL_ROUTING_LAYERS.md`
