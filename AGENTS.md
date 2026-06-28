# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## 个人使用（最小操作面）

- **Python 环境（macOS）**：uv-only、默认 3.12、每仓库 `uv.lock`；禁止 `pip`。重度 Python/ML 任务须高频 `gc.collect()` / `torch.mps.empty_cache()`。
- **Skill Routing**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`。
- **Tool Routing**：PreToolUse/PostToolUse hook 覆盖所有工具（含 MCP）。`ToolOrigin` 分类：NativeHost / McpServer / Unknown。MCP 工具安全审查：`dangerous_mcp_tool_reason()`。

## Lifecycle

- **无固定阶段 lifecycle**。Task 是运行时底层执行引擎（§Task Engine），Loop Goal 是 Task 上的策略与可选自动化层，QG Route 是可插拔质量门注册表。
- **Lifecycle Profile 已从 GOAL_STATE.json 移除**（Wave 2a）。运行时行为由 `RUNTIME_REGISTRY.json` 中的 `lifecycle_profiles` 配置驱动，不写入 GOAL_STATE.json。
- **Review**：Review findings-only。显式 `$code-review-deep` 或 review 请求仍适用。详见 `skills/code-review-deep/SKILL.md`。

## Task Engine（底层执行引擎）

Task 是框架的**底层执行引擎**，不是可选组件。用户层表现为：定义 todo → 执行 todo → 完成 todo。

### v10 三核心概念

| 概念 | 路径 | 职责 |
|------|------|------|
| **Task** | `core/core-state/src/task_state.rs` | 轻量脚手架 — task_id + TASK_LEDGER.jsonl + TASK_POINTERS.json |
| **Loop Goal** | `core/goal-engine/src/` | 可选自动化 — 6 态机 + preflight/dispatch/verify 闭环，只写 GOAL_STATE.json |
| **QG Route** | `core/quality-gate/src/` | 可插拔质量门 — 6 GateChecker，Scene 分发，GateVerdict |

详细架构见 [`ROADMAP-v10.md §2`](docs/ROADMAP-v10.md#2-架构)。真源是 ROADMAP-v10.md（当前架构），非旧的 architecture.md。

### Goal 语义

- 所有 Goal 都是 **Loop** 语义（`goal_type: "loop"` 唯一合法值）。GoalType::Linear 已在 v10 删除。
- 每次 `framework_goal_drive complete` = iteration complete。不归档、不 neutralize 指针，`status` 保持 `running`。

### Quality Gate

- 退出门：双阶段防欺诈（Stage 1 evidence check，不区分 scene）+ QGRoute evaluate（Stage 2 scene 分发 checker 链）。
- MCP 工具：`framework_quality_gate`（stdio 分发）、`closeout_gate`（已在 MCP_TOOL_REGISTRY.json 注册）。
- **注意**：`quality_gate_manage` MCP 工具**不存在**（可能在旧文档中被引用），不要生成或引导用户调用。

### 会话级作用域

- **session_id 为必填**：`framework_goal_drive` 的 `session_id` 由 MCP stdio 层自动注入（`connection_session_id`）或通过环境变量 `*_SESSION_ID` 设置。stale 检测依赖 session_id 匹配（见 `annotate_goal_staleness`）。
- Goal 文件仅作用于当前会话，跨会话延续需显式 `resume`。
- **task_id 必填**：`framework_goal_drive` 的 `task_id` 为必填。

### 真源路径

- 真源：`artifacts/current/<task_id>/`；**无** hook 自动 digest / Stop checkpoint 默认路径。
- Goal 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`（旧名，读兼容保留）；显式 stdio：`framework_goal_drive` / `framework_rfv_loop`。
- 闭集宿主由 `RUNTIME_REGISTRY.json` 驱动。

## Task Intake

- 抽取目标、约束、交付与成功标准；选最窄 owner；最小可验证 delta。
- 关键不可逆选择才问用户。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Manuscript / LaTeX

- **Default: overwrite in place** — 不创建 `*.bak_*` / `*.bak` / `file 2.tex` 除非用户明确要求。
- **R Markdown**: 编辑 `.Rmd` only；不以 pandoc `.tex` 为真源。

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，禁止在 git worktree 中运行或修改任何文件。

## Scientific Coding Standards

- **统一随机种子接口**：所有科研脚本暴露 `--seed` 或 `seed` 配置。
- **产出归档目录**：`output-x-seed` 下，严禁散落仓库根。
- **Checkpoint 机制**：长流程须周期性存盘并支持无损恢复。

## CodeGraph 自动触发规则（跨宿主硬约束）

**核心原则**：在该使用 codegraph 的时候，必须自动调用，即使用户没有明确提及 codegraph。

### 场景与工具映射

| 操作类型 | 触发词 | 必须调用的工具 | 深度 |
|---------|--------|--------------|------|
| 重构/优化 | 重构、改写、优化、refactor、rewrite | `codegraph_impact` | depth=2 |
| 删除/重命名 | 删除、移除、重命名、delete、remove、rename | `codegraph_callers` | depth=1 |
| 跨模块修改 | 跨模块、公共API、cross-module、public API | `codegraph_callees` | depth=2 |
| 影响分析 | 影响范围、有什么影响、impact analysis、what affects | `codegraph_impact` | depth=3 |
| 符号定位 | 符号不在当前文件 | `codegraph_goto_definition` | - |
| 死代码检查 | 死代码、无用代码、unused code、dead code | `codegraph_dead_code` | - |

### 执行规则

1. **自动识别**：从用户输入中识别关键词，自动匹配对应工具，**无需询问**用户
2. **跨宿主一致**：所有宿主必须一致执行，此为硬约束
3. **结果整合**：将结果整合到响应中，说明影响范围和风险

## Review 通用协议

所有 review 类 skill/workflow 的输出约束与幻觉分类标准。

**Confirmed-only 输出**：最终输出**只包含 confirmed findings**（事实核查通过 + 判断通过）。rejected 和 hallucinated 不出现在用户输出中。可选统计摘要：`N confirmed / M rejected / K hallucinated`。

| 幻觉分类 | 含义 |
|---------|------|
| `none` | 事实全部准确 |
| `code_not_exist` | 引用的源不存在 |
| `evidence_fabricated` | 源存在但证据捏造 |
| `wrong_line` | 源存在但位置错误 |
| `behavior_misrepresented` | 证据正确但描述有误 |
| `evidence_out_of_context` | 证据真实但与 finding 无关 |
| `source_moved` | 源已重命名/移动 |
| `partial_hallucination` | 部分准确部分幻觉 |
| `indeterminate` | 无法确认 |

**降级策略**：Factcheck 不可用时单 finding 标记 `indeterminate` 不进 Verify；全阶段失败则所有 finding indeterminate，最终输出为空。不降级为"跳过 factcheck 直接进 Verify"。
