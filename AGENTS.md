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
- **Tool Routing**: 使用 `route_tool(query)` / `search_tools(query, top_k)` 查找 MCP 工具。
  - `skill_route` 现在会返回 `recommended_tools` 字段（与命中 skill 同域的工具列表）。
  - Agent 优先用 `skill_route` 定位 skill，然后参考 `recommended_tools` + 读 SKILL.md 进行后续操作。
  - 工具注册表见 `configs/framework/MCP_TOOL_REGISTRY.json`。

## 路由规则（防回归）

**Slash commands (`/name`) 与自然语言搜索的路由分离**，避免 MCP 路由层截获原生 command：

| 输入类型 | 分辨率 | 路由路径 |
|----------|--------|----------|
| `/gitx`、`/goalx` 等显式 slash command | `~/.claude/skills/<name>/SKILL.md` (Claude 原生) | 不经过 `skill_route` MCP 工具 |
| 自然语言：「帮我提交代码」「检查 goal 状态」 | `skill_route` MCP 工具 | 经 `SKILL_ROUTING_RUNTIME.json` 匹配 |

slash command 对应的 skill 如果同时存在于 `~/.claude/skills/`（原生）和 `RUNTIME_REGISTRY.json` 的 `framework_commands` 中，会导致路由双注册 → MCP dispatch 失败 → `/router-rs-framework:<name> isn't a recognized command`。修复方案：

1. **`~/.claude/skills/<name>/SKILL.md`** → 负责 slash command（唯一注册位）
2. **`RUNTIME_REGISTRY.json`** `framework_commands` → 只保留从未用于原生 slash command 的框架入口
3. **`SKILL_ROUTING_RUNTIME.json`** → 只保留自然语言意图匹配，如需要

**新增 personal level command 的 checklist：**
- [ ] 创建 `~/.claude/skills/<name>/SKILL.md`
- [ ] 检查 `RUNTIME_REGISTRY.json` 的 `framework_commands` 无同名条目
- [ ] 检查 `SKILL_ROUTING_RUNTIME.json` 的 `skills` 无同名条目（除非需要 NL 路由）
- [ ] `framework.md` 路由提示确认 slash command 走原生解析

## Skill 目录

全量活跃 skill 表（39 项），按场景分组。通过 `skill_route(query)` 路由到最佳匹配；详情通过 `skill_read(slug)` 读取。

### 📚 Research（9）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$research` | L2 | none | 统一科研前门 — 文献调研、实验设计、手稿审改 |
| `$good-question` | L2 | none | 选题尖锐化：模糊想法 → falsifiable 科研问题 |
| `$good-story` | L2 | none | 故事线诊断：证据 → 科学叙事 |
| `$deep-search` | L3 | approve | 通用深度搜索引擎，多源覆盖+事实核查 |
| `$paper-workbench` | L3 | none | 论文全流程：审稿/返修/rebuttal/写作/投稿 |
| `$citation-management` | L3 | none | 引用格式核查与 BibTeX 管理 |
| `$experiment-reproducibility` | L3 | none | 实验可复现性管理 |
| `$statistical-analysis` | L4 | none | 统计方法选型与解读 |
| `$math-derivation` | L4 | none | 严格数学推导执行 |

### 💻 Code & Review（5）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$code-review-deep` | L2 | none | 对抗式代码审查，按严重性排序的 findings |
| `$simplify` | L2 | none | 三维并行代码简化（reuse/quality/altitude） |
| `$systematic-debugging` | L0 | evidence | 排查未知故障，根因分析后再修复 |
| `$sentry` | L0 | source | 检查 Sentry 生产错误并分类 |
| `$gh-fix-ci` | L0 | source | 排查修复 GitHub Actions PR check 失败 |

### 🔧 Git & CI（3）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$gitx` | L2 | none | Git closeout 工作流：review → fix → commit → merge |
| `$gh-address-comments` | L0 | source | 回复 GitHub PR review comments |
| `$update` | L0 | none | 刷新 docs、git tracking 和 stale repo surfaces |

### 📄 Documents & Office（4）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$doc` | L3 | artifact | 处理 Word .docx 创建、编辑和审查 |
| `$pdf` | L3 | artifact | 布局感知的 PDF 阅读、编辑、修复 |
| `$slides` | L3 | artifact | 创建和编辑 PPT/PPTX 幻灯片 |
| `$spreadsheets` | L3 | artifact | 工作簿原生电子表格路由到合适通道 |

### 🎨 Design & Figures（3）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$design-md` | L3 | artifact | 设计系统 contract 和 visual token 管理 |
| `$visual-review` | L3 | evidence | 基于截图审查渲染页面和视觉元素 |
| `$tikz-paper-figure` | L3 | none | AI/raster draft → 论文级 TikZ 独立插图 |

### 🏗️ Infrastructure（6）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$research-workspace` | L3 | none | 研究工作区 CLI — claim/假设/实验记录 |
| `$mcp-server-management` | L3 | none | 创建、配置、调试和注册 MCP server |
| `$python-env-management` | L4 | none | macOS Python 治理（uv-only） |
| `$plan-mode` | L1 | none | Plan 门控：证据先行、可验收 todo |
| `$deepinterview` | L1 | none | 证据驱动的需求澄清与收敛 review |
| `$goalx` | L1 | none | Goal 管理生命周期入口 |

### ⚙️ Framework & Core（7）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$agent-swarm-orchestration` | L0 | delegation | 多 agent 编排决策：local / sidecar / team |
| `$skill-framework-developer` | L0 | none | 跨宿主路由调优、框架行为配置 |
| `$primary-runtime` | L0 | none | 框架运行时编排（仅供框架内部使用） |
| `update` | L0 | none | 刷新 docs、git 和 repo 表面 |

### ✅ Quality Gates（6）

| Skill | Layer | Gate | 说明 |
|-------|-------|------|------|
| `$formal-verification` | L4 | none | CAS identity、SMT 一致性、量纲验证 |
| `$literature-verification` | L4 | none | DOI 可达性、引用声明对齐 |
| `$prose-verification` | L4 | none | 术语一致性、style guide、claim drift |
| `$reproducibility-verification` | L4 | none | 实验可复现性：种子、环境、数据版本 |
| `$statistical-verification` | L4 | none | 统计结果验证：p 值、GRIM、effect size |
| `$structure-verification` | L4 | none | LaTeX 编译、交叉引用、格式合规 |

### Routing 指引

```
选择合适的方法：
- 我知道要做什么任务 → skill_route(query)
  → 响应含 recommended_tools（同域工具列表）优先使用
  → 响应含 skill_summary（SKILL.md 摘要）读完即用
  → 否则调用 skill_read(SKILL.md) 获取完整指引
- 我想搜索有什么可用 → skill_search(query, limit)
- 我需要单个 MCP 工具 → route_tool(query)
- 我想探索工具生态 → search_tools(query, top_k)
```

工具注册表见 `configs/framework/MCP_TOOL_REGISTRY.json`。

## Structured Task Output（TASK_OUTPUT.json）

每个 task 完成后自动产出 `TASK_OUTPUT.json`（schema `task-output-v1`），位于 `artifacts/current/<task_id>/TASK_OUTPUT.json`。

### MCP 工具

| 工具 | 功能 |
|------|------|
| `task_output_write` | 写入 TASK_OUTPUT.json（含 closeout 嵌入） |
| `task_output_read` | 读取 TASK_OUTPUT.json |
| `task_output_init` | 初始化空的运行中 TASK_OUTPUT.json |
| `task_output_pull` | 拉取前置 task 输出到当前 task（`consumed_inputs`） |
| `task_output_validate` | 校验 TASK_OUTPUT.json 字段完整性 |
| `chain_aggregate` | 手动触发生成 CHAIN_OUTPUT.json |

**自动集成**：`task_create` 自动 init；`closeout_record_write` 自动同步 closeout → TASK_OUTPUT。

### 输出字段

```
outputs.changed_files        — Vec<String>
outputs.verification_status  — "passed"|"failed"|"partial"|"not_run"
outputs.summary              — String
closeout                     — closeout-record-v1 (完成时嵌入)
consumed_inputs              — Vec<{source_task_id, fields, ...}>
aggregates                   — {parent_chain_id, chain_index}
```

## Chain Engine DAG（task 级编排）

`chain_dag_*` 工具族用于创建和管理带有依赖图的 DAG 链。`TASK_CHAIN.json` 格式扩展支持 `chain-dag-v1`，向后兼容旧格式。

### MCP 工具

| 工具 | 功能 |
|------|------|
| `chain_dag_init` | 创建 DAG 链（含循环检测 + 引用验证） |
| `chain_dag_tick` | 手动推进一次 DAG 调度 |
| `chain_dag_status` | 读完整链状态视图（含拓扑层级） |
| `chain_dag_retry` | 手动重试特定 task |
| `chain_dag_skip` | 跳过特定 task |
| `chain_dag_resume` | 恢复 paused DAG |

### TASK_CHAIN.json DAG Schema

```json
{
  "schema_version": "chain-dag-v1",
  "chain_id": "fix-bugs",
  "mode": "dag",
  "tasks": [{
    "task_id": "scan",
    "depends_on": [],
    "condition": null,
    "parallel_group": null,
    "retry": { "max_attempts": 3 }
  }],
  "global_config": {
    "max_concurrent_tasks": 4,
    "on_any_failure": "pause_dag"
  }
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `depends_on` | `Vec<String>` | 上游 task_id 列表 |
| `condition` | `DagCondition` | 条件门控：`{source, type, field, operator, value}` |
| `parallel_group` | `Option<String>` | 同组 task 可并行执行 |
| `timeout_group` | `TimeoutGroupSpec` | 超时组：`{group_id, max_seconds}` |
| `retry` | `RetryPolicy` | 重试策略：`{max_attempts, backoff_base_ms, ...}` |

### 调度算法

幂等轮询，每次完整重算 DAG 状态（崩溃安全）：
1. expired backoff → pending
2. 收集所有可调度 task（依赖满足 + 条件通过）
3. 按 parallel_group round-robin 公平选择，达 capacity 停止
4. 超时组到期 → failed；失败策略 → abort/pause/continue

