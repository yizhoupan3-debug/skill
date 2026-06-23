# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。各宿主行为差异见 `## 宿主行为差异`。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## 个人使用（最小操作面）

- **Python 环境（macOS）**：uv-only、默认 3.12、每仓库 `uv.lock`；禁止 `pip`。重度 Python/ML 任务须高频 `gc.collect()` / `torch.mps.empty_cache()`。
- **Skill Routing**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`。
- **Tool Routing**：PreToolUse/PostToolUse hook 覆盖所有工具（含 MCP）。`ToolOrigin` 分类：NativeHost / McpServer / Unknown。MCP 工具安全审查：`dangerous_mcp_tool_reason()`。四宿主 matcher 策略见 `docs/research/routing-contracts.md`。

## Lifecycle

- **Default lifecycle**：`/discussx` → `/planx` → `/implementx` → `/verifyx`。详见 `docs/adr/010-ideal-architecture-v10.md`（L0–L7 八层分层模型与生命周期定位）。
- **Review**：Review findings-only。显式 `$code-review-deep` 或 review 请求仍适用。详见 `skills/code-review-deep/SKILL.md`。
- **Closeout**：`closeout_gate` / `complete` 为 advisory（`interactive`）。

## Continuity artifacts（手动画板 only）

- 真源：`artifacts/current/<task_id>/`；**无** hook 自动 digest / `GOAL_CONTINUE` / Stop checkpoint 默认路径。
- Goal 磁盘：`GOAL_STATE.json` / `QUALITY_GATE_STATE.json`；显式 stdio：`framework_goal_drive` / `framework_quality_gate`。
- **会话级作用域**：Goal state 仅作用于当前对话 session，不做跨对话持久化。新 session 首次 `goal_state_manage operation=start` 创建新 state，不读取旧 session 残留。跨 session 延续需用户显式 `resume`。
  - **MCP harness 自动注入**：MCP stdio 层在连接建立时生成 `connection_session_id`（`{host_id}-{nanos}`），自动注入到 `goal_state_manage` 和 `quality_gate_manage`的 payload 中。宿主无需设置环境变量，无需显式传 `session_id` 参数。
  - **task_id 必填**：`goal_state_manage` 的 `task_id` 为必填参数（schema `required` 与代码双重校验）。`closeout_gate` / `goal_state_read` / `quality_gate_status`的 `task_id` 仍为可选（默认 active task）。
- 历史 env 名见 [`MIGRATION.md`](MIGRATION.md) 迁移记录。

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

所有宿主（Claude / Cursor / Codex / OpenCode）必须一致执行。

**核心原则**：在该使用 codegraph 的时候，必须自动调用，即使用户没有明确提及 codegraph。

### 必触发场景（无条件强制执行）

#### 1. 重构/优化操作
**触发词**：重构、改写、优化、改进、重写、refactor、rewrite、optimize
**动作**：调用 `codegraph_impact["目标符号", depth=2]`

#### 2. 删除/重命名操作
**触发词**：删除、移除、重命名、去掉、删掉、delete、remove、rename
**动作**：调用 `codegraph_callers["目标符号", depth=1]`

#### 3. 跨模块修改
**触发词**：跨模块、公共API、公共函数、公共接口、cross-module、public API
**动作**：调用 `codegraph_callees["目标符号", depth=2]`

#### 4. 影响分析
**触发词**：影响范围、影响分析、有什么影响、会影响什么、impact analysis、what affects
**动作**：调用 `codegraph_impact["相关符号", depth=3]`

### 建触发场景（条件触发）

#### 5. 符号定位（当符号不在当前文件时）
**触发条件**：用户提到不在当前文件的符号
**动作**：调用 `codegraph_goto_definition["符号名"]`

#### 6. 死代码检查
**触发词**：死代码、无用代码、unused code、dead code
**动作**：调用 `codegraph_dead_code[language=对应语言]`

### 触发执行规则

1. **自动识别**：从用户输入中识别上述关键词，自动匹配对应工具
2. **无需询问**：直接调用工具，不需要询问用户是否要使用 codegraph
3. **结果整合**：将工具结果整合到响应中，说明影响范围和风险
4. **强制执行**：所有宿主必须一致执行

### 工具映射表

| 操作类型 | 触发词 | 必须调用的工具 | 深度 |
|---------|--------|--------------|------|
| 重构/优化 | 重构、改写、优化 | `codegraph_impact` | depth=2 |
| 删除/重命名 | 删除、移除、重命名 | `codegraph_callers` | depth=1 |
| 跨模块修改 | 跨模块、公共API | `codegraph_callees` | depth=2 |
| 影响分析 | 影响范围、有什么影响 | `codegraph_impact` | depth=3 |
| 符号定位 | 符号不在当前文件 | `codegraph_goto_definition` | - |
| 死代码检查 | 死代码、无用代码 | `codegraph_dead_code` | - |

### 重要说明

- **跨宿主一致性**：所有宿主必须一致执行此规则，不能有宿主差异
- **技能无关性**：无论是否触发了特定技能（如`/implementx`），都必须执行此规则
- **强制性**：这是硬约束，不是建议，所有宿主必须遵守
- **自动触发**：不需要用户显式提及 codegraph，系统应自动识别并调用

## Review 通用协议

所有 review 类 skill/workflow 的输出约束与幻觉分类标准。

### 约束：Confirmed-only 输出

最终用户可见输出**只包含 confirmed findings**。confirmed = 事实核查通过（evidence 真实存在且准确）+ 判断通过（是真实问题）。rejected（判断驳回）和 hallucinated（事实核查拦截）不出现在用户输出中。可选统计摘要行：`N confirmed / M rejected / K hallucinated`。

### 幻觉分类标准（hallucination_type）

| 值 | 含义 |
|----|------|
| `none` | 事实全部准确 |
| `code_not_exist` | 引用的源不存在 |
| `evidence_fabricated` | 源存在但证据捏造/复述 |
| `wrong_line` | 源存在但位置错误 |
| `behavior_misrepresented` | 证据正确但行为/现象描述有误 |
| `evidence_out_of_context` | 证据真实但与 finding 无关 |
| `source_moved` | 源已重命名/移动 |
| `partial_hallucination` | 部分准确部分幻觉 |
| `indeterminate` | 无法确认 |

### 降级策略

Factcheck 工具不可用时：单 finding 标记 `indeterminate` 不进 Verify；全阶段失败则所有 finding 标记 indeterminate，最终输出为空（0 confirmed）。关键：factcheck 整体失败时**不降级为"跳过 factcheck 直接进 Verify"**。

## 宿主行为差异

> 以下段落仅对该节标题标注的宿主生效。通用规则（Language → CodeGraph）对所有宿主一致。

### Claude

- **PreToolUse 硬阻断**：未物化 `GOAL_STATE.json` 或未授权执行区 → 硬阻断。遭遇阻断时 `/discussx` 或 `/planx` 自愈，勿盲目重试。
- **Review gate**：Stop `REVIEW_GATE` advisory-only；**无** `rg_clear` 粘贴面（须完成可数 reviewer lane 或自然语言 override）。
- **框架命令流**：无 `AG_FOLLOWUP` / `updateCurrentStep`；续跑 `framework_goal_drive` + `artifacts/current/<task_id>/` 手动画板。
- **interactive**：suppress spawn-first 与 review Stop nudge（skill 层 findings-only 仍适用）。

### Cursor

- **Hook 事件**：`.cursor/hooks.json` + `router-rs cursor hook`（7 事件闭集）；清门 **Claude canonical**；Stop **advisory-only**。
- **机读短码**：`REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP`（须 `router-rs ` 前缀）；**interactive** suppress `REVIEW_GATE` / `AG_FOLLOWUP`。清门粘贴 **`rg_clear`** 或拒因 token。
- **`updateCurrentStep`**：禁止空载荷；须含可机读步骤或状态。
- **子代理模型**：并行 `Task` 默认继承主会话（省略 `model`）。

### Codex

- **策略嵌入**：编译期 `include_str!` 嵌入本文件（`policy_embed.rs` → `codex_agent_policy`）；hook 运行期不读盘。
- **Hook**：`.codex/hooks.json` + `router-rs codex hook`；清门 **Claude canonical**；Stop **advisory-only** `CODEX_REVIEW_GATE`。
- **多代理**：`/implementx` 且 `execution_mode=parallel` 时应 spawn lane；深度 review spawn-first（`fork_context=false`）。
- **stdio 替代 MCP 工具**：`framework_goal_drive` / `framework_quality_gate`；证据 PostTool 追加。

### OpenCode

- **插件 hook + MCP 双通道**：通过 JS/TS 插件系统提供 hook（`tool.execute.before`、`tool.execute.after`、`session.idle` 等），同时通过 `opencode.json` → MCP 提供框架工具。
- **Review / closeout**：清门 **Claude canonical**；Stop review **advisory-only**（MCP `ADVISORY`）；非 interactive 时 MCP 可对**未满足 closeout 证据** hard-block。
- **权限策略**：**fail-open**（插件层；hook 脚本层对 critical events 仍 fail-closed）。
- **安装**：`framework host-integration install --to opencode --repo-root "$PWD"`。
