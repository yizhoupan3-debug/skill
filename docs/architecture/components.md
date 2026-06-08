---
last_verified: "2026-06-02"
depends_on:
  - ../harness_architecture/index.md
  - ../rust_contracts/index.md
  - ../framework_operator_primer.md
---

# 组件详解

本文档覆盖仓库核心组件：skill 体系、Rust 控制平面、状态管理层、配置目录。

## 1. `skills/` 目录结构

### 1.1 Skill 分层

| 层级 | 含义 | 示例 |
|------|------|------|
| L0 | 用户可直接调用的入口 skill | `implementx`、`verifyx`、`discussx`、`planx` |
| L1 | 工作流内部 skill（被 L0 编排） | `design-md`、`gitx` |
| L2 | 专业能力切片（被 L0/L1 调用） | `paper-reviewer`、`paper-writing`、`code-review-deep` |
| L3 | 基础设施工具 skill | `update`、`skill-framework-developer` |

`routing_layer` 字段在每个 `SKILL.md` 的 frontmatter 中声明。

### 1.2 Skill 文件约定

每个 skill 是 `skills/<name>/SKILL.md`，frontmatter 包含：

- `name`：skill 标识符
- `description`：路由匹配用的自然语言描述
- `routing_layer`：L0/L1/L2/L3
- `routing_owner`：谁拥有这个 skill（`owner` 或具体名称）
- `routing_gate`：前置门控条件（`none` 表示无门控）
- `user-invocable`：用户是否可直接调用
- `trigger_hints`：触发关键词列表

正文包含 `## When to use`、`## Do not use`、具体工作流和输出规范。

### 1.3 路由真源

| 文件 | 角色 | 修改方式 |
|------|------|----------|
| `SKILL_ROUTING_RUNTIME.json` | **热路由真源**（hook 每轮读取） | 手维护 |
| `SKILL_MANIFEST.json` | 冷热混合 manifest（CI/契约消费） | 手维护 |
| `SKILL_ROUTING_METADATA.json` | 路由 metadata（policy 测试消费） | `refresh --write-companions` 再生 |
| `SKILL_PLUGIN_CATALOG.json` | 插件目录（契约校验消费） | `refresh --write-companions` 再生 |
| `SKILL_ROUTING_RUNTIME_EXPLAIN.json` | 路由解释（人读/CI） | `refresh --write-companions` 再生 |

修改 skill 后的刷新流程：

```bash
# 1. 手改 SKILL_ROUTING_RUNTIME.json 和 SKILL_MANIFEST.json
# 2. 再生 companion
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write --write-companions
# 3. 校验
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills validate --framework-root "$PWD"
cargo test --test policy_contracts
```

## 2. `core/router-rs`：Rust 控制平面

### 2.1 职责

`router-rs` 是框架的核心 Rust 二进制，承担：

- **Hook 分发**：接收各宿主的 hook 事件（SessionStart、UserPromptSubmit、PostToolUse、Stop 等），执行策略判定，输出 JSON 给宿主。
- **Skill 路由**：读取 `SKILL_ROUTING_RUNTIME.json`，根据用户意图匹配 skill。
- **门控执行**：REVIEW_GATE（深度审稿门控）、closeout 门控、pre-goal 检查。
- **框架维护**：`framework skills validate/refresh`、`framework host-integration install`、`framework doctor`、`framework maint update-one-shot`。
- **MCP stdio 服务**：`framework_goal_drive`、`framework_rfv_loop`、`trace_runtime` 等 stdio 协议。
- **Browser MCP**：`router-rs browser mcp-stdio` 子命令。

### 2.2 源码结构

```
core/router-rs/src/
  main.rs              # CLI 入口，插入 "host" 子命令前缀
  lib.rs               # 模块声明，re-export antigravity-core 的类型
  cli/                 # clap CLI 定义与分发
  hosts/
    codex_hooks/       # Codex CLI hook 处理
    cursor_hooks/      # Cursor hook 处理（handlers.rs + handlers_parts/）
    claude_code_hooks.rs    # Claude Code hook 处理
    mcp_stdio_harness.rs  # MCP stdio harness（Antigravity / Opencode）
  route/               # 路由引擎（records.rs、routing.rs、scoring.rs、signals.rs）
  framework_runtime/   # 框架运行时视图
  runtime_registry/    # 宿主 registry 磁盘 loader
  browser_mcp/         # Browser MCP 服务
  utils/               # 路径守卫、原子写、task ledger flock 锁
```

### 2.3 CLI 入口

`router-rs` 二进制通过 `main.rs` 接收命令行参数。当第一个参数是闭集宿主名（`codex`/`claude-code`/`cursor`/`antigravity`/`opencode` 等）时，自动插入 `host` 子命令前缀。

主要命令组：

| 命令 | 用途 |
|------|------|
| `router-rs host codex hook` | Codex hook 分发 |
| `router-rs host cursor hook` | Cursor hook 分发 |
| `router-rs host claude hook` | Claude Code hook 分发 |
| `router-rs host opencode agent` | OpenCode MCP agent |
| `router-rs framework skills validate` | 校验 skill 路由产物一致性 |
| `router-rs framework skills refresh` | 刷新 skill 路由产物 |
| `router-rs framework host-integration install` | 安装宿主 hook 配置 |
| `router-rs framework doctor` | 框架健康检查 |
| `router-rs framework maint update-one-shot` | 全量维护（校验 + 刷新 + sync） |
| `router-rs framework sync-entrypoints` | 同步 AGENTS.md 宿主投影 |
| `router-rs framework goal-drive` | Goal state stdio 协议 |
| `router-rs framework rfv-loop` | RFV 多轮 stdio 协议 |

### 2.4 关键机制

**Review Gate（Stop advisory-only）**：独立 reviewer 证据未满足时，Stop 仅注入 advisory nudge（`followup_message` 等；全宿主不 `permission: deny` / `decision:block`）。清门真源：`independent_reviewer_seen` 或 override（`core-policy::review_gate_satisfied`）。lane 闭集定义在 `RUNTIME_REGISTRY.json` 的 `review_gate.reviewer_lanes`。

**Closeout 门控（与 review 分层）**：closeout 需要证据（`EVIDENCE_INDEX.json`）、diff、产物和明确 blocker；`ROUTER_RS_CLOSEOUT_ENFORCEMENT` 等可 fail-closed，与 review advisory 无关。本地默认软门禁，CI 硬门禁。

**Task ledger flock**：`artifacts/current/.router-rs.task-ledger.lock` 上的 `flock(2)` 保证多宿主 hook 子进程对 `GOAL_STATE.json`、`RFV_LOOP_STATE.json`、`STEP_LEDGER.jsonl` 的写入互斥。

**出站裁剪**：Cursor hook 的 `additional_context` 按 UTF-8 字节预算截断（默认 8192），优先保留 `REVIEW_GATE` 等硬短码行。

## 3. `core/antigravity`：状态管理层

`antigravity-core` 是一个纯库 crate，被 `router-rs` 依赖，提供：

| 模块 | 文件 | 职责 |
|------|------|------|
| `task_state` | `task_state.rs` (59KB) | `ResolvedTaskView` 解析、active/focus/supervisor 任务选择 |
| `state_manager` | `state_manager.rs` (90KB) | Goal/RFV 状态管理、stdio 协议实现 |
| `rfv_loop` | `rfv_loop.rs` | RFV 多轮循环状态 |
| `step_ledger` | `step_ledger.rs` | 长任务 step 恢复流 |
| `task_ledger` | `task_ledger.rs` | Task 级 ledger 抽象 |
| `task_state_aggregate` | `task_state_aggregate.rs` | `TASK_STATE.json` 聚合投影 |

状态文件存储在 `artifacts/current/<task_id>/`：

- `GOAL_STATE.json`：当前目标状态
- `RFV_LOOP_STATE.json`：RFV 循环状态
- `EVIDENCE_INDEX.json`：验证证据索引
- `STEP_LEDGER.jsonl`：长任务步骤记录
- `TRACE_EVENTS.jsonl`：轨迹诊断流
- `WAVE_STATE.json`：implementx 的 wave 执行状态
- `ROADMAP.md`：规划产物

## 4. `configs/` 目录

### 4.1 `configs/framework/`

| 文件 | 用途 | 热/冷 |
|------|------|-------|
| `RUNTIME_REGISTRY.json` | 闭集宿主、review gate lane、profile 投影 | 热（运行时磁盘读取） |
| `host_projection_narrative.json` | 宿主投影内 My lifecycle 和 review 文案 | 冷（安装时读取） |
| `GENERATED_ARTIFACTS.json` | 声明纳入版本库的生成物路径和 generator 命令 | 冷（维护时读取） |
| `HARNESS_OPERATOR_NUDGES.json` | operator 提示文案配置 | 冷 |
| `HARNESS_FAILURE_TAXONOMY.json` | 失败分类机器可读表 | 冷 |
| `HARNESS_BEHAVIORAL_EVAL_CASES.json` | 行为评估 fixture | 冷 |
| `CLOSEOUT_RECORD_SCHEMA.json` | closeout record JSON schema | 冷 |
| `RUNTIME_REGISTRY_SCHEMA.json` | registry JSON schema | 冷 |
| `NL_ROUTE_ADJUSTMENTS.json` | 自然语言路由调整规则 | 热（路由引擎消费） |
| `REVIEW_ROUTING_SIGNALS.json` | 审稿路由信号定义 | 热 |
| `PAPER_ADVERSARIAL_HOOK.txt` | 论文对抗审稿 hook 文案 | 冷（注入时读取） |
| `PAPER_PROSE_QUALITY_HOOK.txt` | 论文写作质量 hook 文案 | 冷 |
| `FRAMEWORK_SURFACE_POLICY.json` | 框架表面策略 | 冷 |
| `OPERATOR_PROFILE_SOLO.env` | 单人 operator 环境变量模板 | 冷 |
| `cursor-router-rs-hook.sh` | Cursor hook launcher 脚本 | 热（每事件调用） |
| `claude-router-rs-hook.sh` | Claude hook launcher 脚本 | 热 |
| `codex-router-rs-hook.sh` | Codex hook launcher 脚本 | 热 |
| `cursor-hooks.workspace-template.json` | 跨仓库 Cursor hooks 模板 | 冷 |

### 4.2 `configs/codex/` 和 `configs/framework/` 的关系

`configs/codex/` 包含 Codex 特定配置（如 aggregator 配置）。`configs/framework/` 是跨宿主的框架配置。
