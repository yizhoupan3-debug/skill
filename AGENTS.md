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

## 路由规则

| 输入类型 | 路由路径 |
|----------|---------|
| `/gitx`、`/goalx` 等 slash command | `~/.claude/skills/<name>/SKILL.md`（Claude 原生，不经过 MCP） |
| 自然语言任务（统一入口） | `skill_route(query)` → 响应含 `recommended_tools` + `selected_skill` + `skill_summary` |
| 探索 MCP 工具生态 | `search_tools(query, top_k)` |

**新增 personal command checklist**: 创建 `~/.claude/skills/<name>/SKILL.md` → 确认 `configs/framework/RUNTIME_REGISTRY.json` 无同名 → 确认 `skills/SKILL_ROUTING_RUNTIME.json` 无同名（除非需 NL 路由）。

工具注册表见 `configs/framework/MCP_TOOL_REGISTRY.json`。

---

## Harness 速查卡（模型行动指南）

**遇到以下场景，主动使用对应工具。不要等用户提醒。**

### A. 科研与研究

| 场景 | 行动 |
|------|------|
| 文献调研 / 论文审改 / 实验设计 | `skill_route("research...")` 或 `$research` |
| 选题尖锐化 / 模糊想法→问题 | `skill_route("good question")` |
| 故事线诊断 / 结果→叙事 | `skill_route("good story")` |
| 深度网络搜索 + 事实核查 | `skill_route("deep search")` |
| 引用管理 / BibTeX | `skill_route("citation")` |

### B. 数学与验证

| 场景 | 行动 |
|------|------|
| 验证数学推导/证明 | `math_identity_chain` / `math_z3_prove` / `math_prove_inequality` |
| 求解方程/化简/展开 | `math_sympy_solve` / `math_sympy_simplify` / `math_sympy_expand` |
| 微积分运算 | `math_sympy_integrate` / `math_sympy_differentiate` |
| 极限/级数/渐近 | `math_sympy_limit` / `math_sympy_series` / `math_asymptotic_estimate` |
| 约束系统可满足性 | `math_z3_check_system` / `math_z3_solver_*` |
| 优化问题 | `math_z3_optimize` |
| 正则摄动展开 | `math_perturbation_expand` |
| 量纲一致性 | `math_sympy_dimension_propagate` |
| 证明结构管理 | `math_proof_dag_init` / `math_proof_dag_verify` |

### C. 论文与文档

| 场景 | 行动 |
|------|------|
| 论文审稿/返修/润色 | `skill_route("paper workbench")` |
| Word .docx 创建/编辑 | `skill_route("doc")` |
| PDF 读写/修复 | `skill_route("pdf")` |
| PPT/PPTX 幻灯片 | `skill_route("slides")` |
| Excel/CSV 表格 | `skill_route("spreadsheets")` |

### D. 代码与开发

| 场景 | 行动 |
|------|------|
| 深度代码审查（只审不改） | `skill_route("code review")` |
| 代码简化（双维并行：复用/质量） | `skill_route("simplify")` |
| 根因未知的故障排查 | `skill_route("systematic debugging")` |
| CI 失败修复 | `skill_route("gh-fix-ci")` |
| PR 评论回复 | `skill_route("gh-address-comments")` |

### E. Goal / Task / Chain 管理

| 场景 | 行动 |
|------|------|
| 创建/管理 Goal 生命周期 | `goal_state_manage(operation=start/checkpoint/amend/complete/...)` |
| 创建/推进任务 | `task_create` → `task_focus` → `task_complete` |
| DAG 任务编排 | `chain_dag_init` → `chain_dag_tick` → `chain_dag_status` |
| 记录 closeout | `closeout_record_write` |
| 读写任务输出 | `task_output_write` / `task_output_read` |

### F. 文献与统计验证

| 场景 | 行动 |
|------|------|
| DOI 可达性 / 引用对齐 | `research_verification_literature(check="doi"/"claim_coverage")` |
| 统计声明验证（GRIM/p值） | `research_verification_statistical(check="grim"/"p_value")` |
| 实验可复现性验证 | `research_verification_reproducibility(check=...)` |
| 文本质控（术语/套话） | `research_verification_prose(check=...)` |
| 形式验证（量纲/见证） | `research_verification_formal(check=...)` |

### G. 框架运维

| 场景 | 行动 |
|------|------|
| 路由系统调优/治理 | `skill_route("skill-framework-developer")` |
| MCP server 创建/调试 | `skill_route("mcp server")` |
| 设计系统 contract | `skill_route("design-md")` |
| 视觉审查 | `skill_route("visual review")` |

---

## Skill 目录（43 项）

通过 `skill_route(query)` 路由；详情通过 `skill_read(slug)` 读取。

**Research**: `$research`(统一前门) · `$good-question` · `$good-story` · `$deep-search` · `$paper-workbench` · `$citation-management` · `$experiment-reproducibility` · `$statistical-analysis` · `$financial-data` · `$math-verify` · `$math-explore` · `$math-modeling`

**Code & Review**: `$code-review-deep` · `$simplify` · `$systematic-debugging` · `$sentry` · `$gh-fix-ci`

**Git & CI**: `$gitx` · `$gh-address-comments` · `$update`

**Documents**: `$doc` · `$pdf` · `$slides` · `$spreadsheets`

**Design & Figures**: `$design-md` · `$visual-review` · `$tikz-paper-figure`

**Infrastructure**: `$research-workspace` · `$browser-automation` · `$mcp-server-management` · `$python-env-management` · `$plan-mode` · `$deepinterview` · `$goalx`

**Framework Core**: `$agent-swarm-orchestration` · `$skill-framework-developer` · `$primary-runtime`

**Quality Gates**: `$formal-verification` · `$literature-verification` · `$prose-verification` · `$reproducibility-verification` · `$statistical-verification` · `$structure-verification`

---

## 参考文档

- **Task Output / Chain DAG** 详细文档见 [`AGENTS_REFERENCE.md`](AGENTS_REFERENCE.md)（按需加载，不自动注入上下文）
- **路由分层详解**见 `skills/SKILL_ROUTING_LAYERS.md`
