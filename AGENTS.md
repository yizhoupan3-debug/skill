# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。

## OUTPUT RULES（最高优先级，始终生效）

**输出必须保持干练。** 所有技术内容保留，仅填充词消灭。

规则：drop 冠词(a/an/the)、填充词(just/really/basically)、客套(sure/certainly/happy to)、模棱两可(might be worth/you could consider)。短同义词(big不extensive, fix不implement a solution)。片段句OK。不自称风格。无工具调用旁白。技术术语/代码/error string/API名精确保留。标准缩写OK(DB/API/HTTP)，不发明新缩写(cfg/impl/req—tokenizer不省)。无因果箭头(→)。

模式：`[事] [动] [因]。[下步]。`

不是：「Sure! I'd be happy to help you with that. The issue is likely caused by...」
而是：「Bug in auth middleware. Token expiry check用`<`不`<=`。Fix:」

**每轮持续生效，不随轮数退化。** 安全警告/不可逆操作/用户迷惑时自动恢复详细语气(Auto-Clarity)。子代理/子任务继承。关闭用 `normal mode`。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- 子代理/子任务继承上述语言与输出规则。

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
| 根因未知故障排查 | `$systematic-debugging` |
| CI 失败修复 / PR 评论回复 | `$gh-fix-ci` / `$gh-address-comments` |
| Goal/Task 生命周期 | `goal_state_manage(...)` / `task_create` / `task_focus` / `task_complete` |
| DAG 编排 | `chain_dag_init` / `chain_dag_tick` |
| Closeout 记录 | `closeout_record_write` |
| 文献/统计/实验验证 | `research_verification_*` 系列 |
| 框架运维/路由治理 | `$skill-framework-developer` |
| 设计/原型/品牌 | `$design-md` / `$visual-review` / `$hallmark` / `$huashu-design` |
| MCP server 管理 | `$mcp-server-management` |
| 出版级科研图表 | `$scipilot-figure` / `$tikz-paper-figure` |

---

## Skill 目录

`skill_route(query)` 路由。顶刊专版 + `$ajs-*` 通配符扩展。

`$research` · `$good-question` · `$good-story` · `$deep-search` · `$paper-workbench` · `$citation-management` · `$experiment-reproducibility` · `$statistical-analysis` · `$financial-data` · `$math-verify` · `$math-explore` · `$math-modeling` · `$ajs-*` (9本顶刊) · `$code-review-deep` · `$simplify` · `$systematic-debugging` · `$sentry` · `$gh-fix-ci` · `$gitx` · `$gh-address-comments` · `$update` · `$doc` · `$pdf` · `$slides` · `$spreadsheets` · `$design-md` · `$visual-review` · `$tikz-paper-figure` · `$scipilot-figure` · `$huashu-design` · `$hallmark` · `$research-workspace` · `$web-tools` · `$mcp-server-management` · `$python-env-management` · `$plan-mode` · `$deepinterview` · `$goalx` · `$initx` · `$skill-framework-developer` · `$primary-runtime`

## 参考文档

- **Task Output / Chain DAG** 详见 [`AGENTS_REFERENCE.md`](AGENTS_REFERENCE.md)
- **路由分层详解**见 `skills/SKILL_ROUTING_LAYERS.md`
