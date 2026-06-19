# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。宿主差异见 `AGENTS_<HOST>.md`。

**双文件注入（硬约束）**：各闭集宿主须**同时**注入仓库根 **`AGENTS.md`**（跨宿主内核）与 **`AGENTS_<HOST>.md`**（transport delta）；**禁止**合并为单文件。

**闭集宿主（2026-06）**：`codex`、`claude`、`cursor`、`opencode` — 真源 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。已退役 id：`codex-app`、`codex-cli`。

## 权威分层

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Lifecycle、Closeout） | 仓库根 **`AGENTS.md`** |
| 宿主执行面差异 | `AGENTS_<HOST>.md` + 各宿主 hook/rules |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs` |

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## 个人使用（最小操作面）

- **Python 环境（macOS）**：uv-only、默认 3.12、每仓库 `uv.lock`；禁止 `pip`。重度 Python/ML 任务须高频 `gc.collect()` / `torch.mps.empty_cache()`。
- **Skill Routing**：热入口 `skills/SKILL_ROUTING_RUNTIME.json`；只读命中项 `skill_path`。

## Lifecycle

- **Default lifecycle**：`/discussx` → `/planx` → `/implementx` → `/verifyx`。详见 `docs/spec.md` §6。
- **Review**：Review findings-only。显式 `$code-review-deep` 或 review 请求仍适用。详见 `skills/code-review-deep/SKILL.md`。
- **Closeout**：`closeout_gate` / `complete` 为 advisory（`my-light`）。

## Continuity artifacts（手动画板 only）

- 真源：`artifacts/current/<task_id>/`；**无** hook 自动 digest / `GOAL_CONTINUE` / Stop checkpoint 默认路径。
- Goal/RFV 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`；显式 stdio：`framework_goal_drive` / `framework_rfv_loop`。
- **会话级作用域**：Goal state 仅作用于当前对话 session，不做跨对话持久化。

## 启动序列（跨宿主 DAG）

- **T0 并行**：`framework_snapshot` ∥ `skill_route` ∥ `goal_state_manage(start)` — 无数据依赖，首轮必须。
- **T1 按需**：`record_evidence` — 验证类命令后追加。
- **T2 延迟**：`session_checkpoint` → `closeout_gate` → `goal_state_manage(complete)` — 对话结束时执行，首轮跳过。

## 宿主能力差异（降级矩阵）

| 能力 | claude | cursor | codex | opencode |
|------|:-----------:|:------:|:-----:|:--------:|
| hard gate hooks | ✓ | ✓ | ✓ | ✗ |
| closeout evidence hooks | ✓ | ✓ | ✓ | ✓ |
| review gate observable | ✓ | ✓ | ✓ | ✓ |
| session supervisor | mcp_bridge | ✗ | codex_driver | ✗ |
| worktree | ✓ | ✓ | ✓ | ✗ |
| batch/cron/CI | ✗ | ✗ | ✓ | ✗ |

详见 `configs/framework/RUNTIME_REGISTRY.json` 各宿主 `harness_capability_exceptions`。

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

**核心原则**：在该使用codegraph的时候，必须自动调用，即使用户没有明确提及codegraph。

### 必触发场景（无条件强制执行）

#### 1. 重构/优化操作
**触发词**：重构、改写、优化、改进、重写、refactor、rewrite、optimize
**动作**：调用 `codegraph_impact["目标符号", depth=2]`
**示例**：
```
用户：帮我重构这个函数
LLM：（自动调用codegraph_impact分析影响范围）→ 根据结果制定重构计划
```

#### 2. 删除/重命名操作
**触发词**：删除、移除、重命名、去掉、删掉、delete、remove、rename
**动作**：调用 `codegraph_callers["目标符号", depth=1]`
**示例**：
```
用户：删除这个公共API
LLM：（自动调用codegraph_callers检查调用者）→ 确认无遗漏调用者后执行
```

#### 3. 跨模块修改
**触发词**：跨模块、公共API、公共函数、公共接口、cross-module、public API
**动作**：调用 `codegraph_callees["目标符号", depth=2]`
**示例**：
```
用户：修改这个跨模块函数
LLM：（自动调用codegraph_callees检查下游影响）→ 评估破坏性后执行
```

#### 4. 影响分析
**触发词**：影响范围、影响分析、有什么影响、会影响什么、impact analysis、what affects
**动作**：调用 `codegraph_impact["相关符号", depth=3]`
**示例**：
```
用户：看看这个改动有什么影响
LLM：（自动调用codegraph_impact分析完整影响范围）→ 报告影响范围
```

### 建触发场景（条件触发）

#### 5. 符号定位（当符号不在当前文件时）
**触发条件**：用户提到不在当前文件的符号
**动作**：调用 `codegraph_goto_definition["符号名"]`
**示例**：
```
用户：这个handle_request函数在哪里定义的？
LLM：（自动调用codegraph_goto_definition定位）→ 返回定义位置
```

#### 6. 死代码检查
**触发词**：死代码、无用代码、unused code、dead code
**动作**：调用 `codegraph_dead_code[language=对应语言]`
**示例**：
```
用户：有没有死代码
LLM：（自动调用codegraph_dead_code检查）→ 返回死代码列表
```

### 触发执行规则

1. **自动识别**：从用户输入中识别上述关键词，自动匹配对应工具
2. **无需询问**：直接调用工具，不需要询问用户是否要使用codegraph
3. **结果整合**：将工具结果整合到响应中，说明影响范围和风险
4. **强制执行**：所有宿主（claude、cursor、codex、opencode）必须一致执行

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
- **技能无关性**：无论是否触发了特定技能（如/implementx），都必须执行此规则
- **强制性**：这是硬约束，不是建议，所有宿主必须遵守
- **自动触发**：不需要用户显式提及codegraph，系统应自动识别并调用
