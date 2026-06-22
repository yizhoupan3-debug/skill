# Cross-Host Architecture (跨宿主实现细节)

本文档管理跨宿主的实现细节、差异和架构信息。**AGENTS.md** 只包含执行时需要读取的上下文和工具指引。

## 权威分层

| 类别 | 权威落点 |
|------|----------|
| 跨宿主叙述性协议（语言、路由、Lifecycle、Closeout） | 仓库根 **`AGENTS.md`**（含 `## 宿主行为差异` 附录） |
| 宿主执行面差异 | `AGENTS.md` § 宿主行为差异 + 各宿主 hook/rules |
| skill 路由 | `skills/SKILL_ROUTING_RUNTIME.json` |
| 框架命令 / CLI | `configs/framework/RUNTIME_REGISTRY.json` |
| hook 行为 | 各宿主 `hooks.json` + `router-rs` |

## 闭集宿主

**闭集宿主（2026-06）**：`codex`、`claude`、`cursor`、`opencode` — 真源 `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`。已退役 id：`codex-app`、`codex-cli`。

## 单文件策略

`AGENTS.md` 是唯一的策略真源文件，包含跨宿主通用规则和 `## 宿主行为差异` 附录（收纳各宿主 transport delta）。各宿主通过 Rust `host-projection` crate 将 `AGENTS.md` 注入到宿主 context 中，不再维护独立的 delta 文件。

**实现细节**：
- `AGENTS.md`：跨宿主内核 + 宿主行为差异附录，是唯一的策略真源
- 各宿主通过 `context_file()` 返回 `"AGENTS.md"`，projection 系统统一注入

## 宿主能力差异（降级矩阵）

| 能力 | claude | cursor | codex | opencode |
|------|:-----------:|:------:|:-----:|:--------:|
| hard gate hooks | ✓ | ✓ | ✓ | ✓ |
| closeout evidence hooks | ✓ | ✓ | ✓ | ✓ |
| review gate observable | ✓ | ✓ | ✓ | ✓ |
| session supervisor | mcp_bridge | ✗ | codex_driver | ✗ |
| worktree | ✓ | ✓ | ✓ | ✗ |
| batch/cron/CI | ✗ | ✗ | ✓ | ✗ |

详见 `configs/framework/RUNTIME_REGISTRY.json` 各宿主 `host_projections`。

## 启动序列（跨宿主 DAG）

### T0 并行（首轮必须）
- `framework_snapshot`
- `skill_route`
- `goal_state_manage(start)`

无数据依赖，首轮必须并行执行。

### T1 按需
- `record_evidence` — 验证类命令后追加

### T2 延迟（对话结束时执行）
- `session_checkpoint`
- `closeout_gate`
- `goal_state_manage(complete)`

首轮跳过，对话结束时执行。

## 跨宿主一致性要求

### 1. 语言一致性
- 面向用户的回复必须使用简体中文
- 仅当用户当轮明确要求英文时才可切换

### 2. CodeGraph 自动触发一致性
**所有宿主必须一致执行**：在该使用codegraph的时候，必须自动调用，即使用户没有明确提及codegraph。

**必触发场景**：
- 重构/优化操作 → `codegraph_impact`
- 删除/重命名操作 → `codegraph_callers`
- 跨模块修改 → `codegraph_callees`
- 影响分析 → `codegraph_impact`

**建触发场景**：
- 符号定位 → `codegraph_goto_definition`
- 死代码检查 → `codegraph_dead_code`

详见 `AGENTS.md` § CodeGraph 自动触发规则。

### 3. Lifecycle 一致性
- Default lifecycle：`/discussx` → `/planx` → `/implementx` → `/verifyx`
- Review：Review findings-only
- Closeout：`closeout_gate` / `complete` 为 advisory（`interactive`）

### 4. 工具使用一致性
- 所有宿主必须使用相同的工具集
- 工具调用方式和参数必须一致
- 错误处理和响应格式必须一致

## 宿主特定差异

### Claude
- PreToolUse 硬阻断（独有）
- Review gate（canonical）
- interactive：suppress spawn-first 与 review Stop nudge

### Cursor
- Hook：`.cursor/hooks.json` + `router-rs cursor hook`（7 事件闭集）
- 机读短码：`REVIEW_GATE`、`AG_FOLLOWUP`、`CLOSEOUT_FOLLOWUP`
- `updateCurrentStep`：禁止空载荷；须含可机读步骤或状态

### Codex
- 策略嵌入：编译期 `include_str!` 嵌入 `AGENTS.md`（`policy_embed.rs` → `codex_agent_policy`）
- 多代理：`/implementx` 且 `execution_mode=parallel` 时应 spawn lane
- stdio 替代 MCP 工具

### OpenCode
- 插件 hook + MCP 双通道
- Review / closeout：清门 **Claude canonical**
- 权限策略：**fail-open**（插件层；hook 脚本层对 critical events 仍 fail-closed）

## 实现细节

### 路由引擎
- skill 路由：`skills/SKILL_ROUTING_RUNTIME.json`
- 框架命令 / CLI：`configs/framework/RUNTIME_REGISTRY.json`

### Hook 系统
- hook 行为：各宿主 `hooks.json` + `router-rs`
- 事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`

### 状态管理
- 真源：`artifacts/current/<task_id>/`
- Goal/RFV 磁盘：`GOAL_STATE.json` / `RFV_LOOP_STATE.json`
- 会话级作用域：Goal state 仅作用于当前对话 session

## 相关文档

- `AGENTS.md`：跨宿主策略真源（含宿主行为差异附录）
- `docs/spec.md`：框架规范
- `docs/hosts/_common.md`：宿主通用手册
- `docs/hosts/hook-hosts.md`：Hook 宿主手册
