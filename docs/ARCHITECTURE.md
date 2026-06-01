---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture.md
  - rust_contracts.md
  - README.md
---

# Architecture

本文档是仓库唯一入门级架构地图。目标读者：首次接触本仓库的开发者或 AI agent，需要在 10 分钟内理解「这个仓库是什么、各组件怎么连接、修改哪里才安全」。

详细契约、环境变量表、宿主差异见 `docs/` 子文档（索引见 `docs/README.md`）。

## 1. 仓库定位

本仓库是一套面向 AI coding agent（Codex、Cursor、Claude Code、Antigravity）的 skill 路由与执行治理框架。核心目标：

- **Skill 路由**：给定用户意图，从 `skills/` 中选出最窄匹配的 skill，注入对应 prompt。
- **执行治理**：通过 hook 和 Rust 控制平面，约束 agent 在 review、closeout、goal drive 等关键节点的行为。
- **多宿主适配**：同一套 skill 和策略，通过宿主投影层适配不同 AI coding 平台。

## 2. `skills/` 目录结构

### 2.1 Skill 分层

| 层级 | 含义 | 示例 |
|------|------|------|
| L0 | 用户可直接调用的入口 skill | `implementx`、`verifyx`、`discussx`、`planx` |
| L1 | 工作流内部 skill（被 L0 编排） | `design-md`、`gitx` |
| L2 | 专业能力切片（被 L0/L1 调用） | `paper-reviewer`、`paper-writing`、`code-review-deep` |
| L3 | 基础设施工具 skill | `update`、`skill-framework-developer` |

`routing_layer` 字段在每个 `SKILL.md` 的 frontmatter 中声明。

### 2.2 Skill 文件约定

每个 skill 是 `skills/<name>/SKILL.md`，frontmatter 包含：

- `name`：skill 标识符
- `description`：路由匹配用的自然语言描述
- `routing_layer`：L0/L1/L2/L3
- `routing_owner`：谁拥有这个 skill（`owner` 或具体名称）
- `routing_gate`：前置门控条件（`none` 表示无门控）
- `user-invocable`：用户是否可直接调用
- `trigger_hints`：触发关键词列表

正文包含 `## When to use`、`## Do not use`、具体工作流和输出规范。

### 2.3 路由真源

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

### 2.4 默认生命周期

```
/discussx  ->  /planx  ->  /implementx  ->  /verifyx
(讨论)        (规划)       (执行全部 wave)    (验收并清理)
```

`implementx` 一口气执行 `WAVE_STATE.json` 中所有 wave；`verifyx` 验收后清理 `artifacts/current/<task_id>/`。

## 3. `core/router-rs`：Rust 控制平面

### 3.1 职责

`router-rs` 是框架的核心 Rust 二进制，承担：

- **Hook 分发**：接收各宿主的 hook 事件（SessionStart、UserPromptSubmit、PostToolUse、Stop 等），执行策略判定，输出 JSON 给宿主。
- **Skill 路由**：读取 `SKILL_ROUTING_RUNTIME.json`，根据用户意图匹配 skill。
- **门控执行**：REVIEW_GATE（深度审稿门控）、closeout 门控、pre-goal 检查。
- **框架维护**：`framework skills validate/refresh`、`framework host-integration install`、`framework doctor`、`framework maint update-one-shot`。
- **MCP stdio 服务**：`framework_goal_drive`、`framework_rfv_loop`、`trace_runtime` 等 stdio 协议。
- **Browser MCP**：`router-rs browser mcp-stdio` 子命令。

### 3.2 源码结构

```
core/router-rs/src/
  main.rs              # CLI 入口，插入 "host" 子命令前缀
  lib.rs               # 模块声明，re-export antigravity-core 的类型
  cli/                 # clap CLI 定义与分发
  hosts/
    codex_hooks/       # Codex CLI hook 处理
    cursor_hooks/      # Cursor hook 处理（handlers.rs + handlers_parts/）
    claude_hooks.rs    # Claude Code hook 处理
    claude_desktop_hooks.rs  # Claude Desktop hook 处理
    antigravity_cli_hooks/   # Antigravity CLI hook 处理
  route/               # 路由引擎（records.rs、routing.rs、scoring.rs、signals.rs）
  framework_runtime/   # 框架运行时视图
  runtime_registry/    # 宿主 registry 磁盘 loader
  browser_mcp/         # Browser MCP 服务
  utils/               # 路径守卫、原子写、task ledger flock 锁
```

### 3.3 CLI 入口

`router-rs` 二进制通过 `main.rs` 接收命令行参数。当第一个参数是宿主名（`codex`/`claude`/`cursor`/`antigravity-cli`/`antigravity-app`）时，自动插入 `host` 子命令前缀。

主要命令组：

| 命令 | 用途 |
|------|------|
| `router-rs host codex hook` | Codex hook 分发 |
| `router-rs host cursor hook` | Cursor hook 分发 |
| `router-rs host claude hook` | Claude Code hook 分发 |
| `router-rs host claude-desktop hook` | Claude Desktop hook 分发 |
| `router-rs framework skills validate` | 校验 skill 路由产物一致性 |
| `router-rs framework skills refresh` | 刷新 skill 路由产物 |
| `router-rs framework host-integration install` | 安装宿主 hook 配置 |
| `router-rs framework doctor` | 框架健康检查 |
| `router-rs framework maint update-one-shot` | 全量维护（校验 + 刷新 + sync） |
| `router-rs framework sync-entrypoints` | 同步 AGENTS.md 宿主投影 |
| `router-rs framework goal-drive` | Goal state stdio 协议 |
| `router-rs framework rfv-loop` | RFV 多轮 stdio 协议 |

### 3.4 关键机制

**Review Gate**：当深度审稿 lane（代码、安全、架构等）的 review 未完成时，Stop 事件会被阻断（Codex `decision:block`）或附加续跑提示（Cursor `followup_message`）。lane 闭集定义在 `RUNTIME_REGISTRY.json` 的 `review_gate.deep_gate_lanes`。

**Closeout 门控**：closeout 需要证据（`EVIDENCE_INDEX.json`）、diff、产物和明确 blocker。本地默认软门禁，CI 硬门禁。

**Task ledger flock**：`artifacts/current/.router-rs.task-ledger.lock` 上的 `flock(2)` 保证多宿主 hook 子进程对 `GOAL_STATE.json`、`RFV_LOOP_STATE.json`、`STEP_LEDGER.jsonl` 的写入互斥。

**出站裁剪**：Cursor hook 的 `additional_context` 按 UTF-8 字节预算截断（默认 8192），优先保留 `REVIEW_GATE` 等硬短码行。

## 4. `core/antigravity`：状态管理层

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

## 5. 宿主适配层

### 5.1 宿主列表

| 宿主 | Hook 入口 | 配置文件 | 文档 |
|------|-----------|----------|------|
| Cursor | `.cursor/hooks.json` (7 事件) | `.cursor/hooks.json` + `.cursor/router-rs-hook.env` | `docs/hosts/cursor.md` |
| Codex CLI | `~/.codex/hooks.json` | `~/.codex/config.toml` | `docs/hosts/codex-cli.md` |
| Claude Code | `.claude/settings.json` (4 事件) | `.claude/settings.json` + `.claude/router-rs-hook.env` | `docs/hosts/claude.md` |
| Claude Desktop | Claude Desktop MCP | `.claude-desktop/` | `docs/hosts/claude-desktop.md` |
| Antigravity | Antigravity CLI hooks | `.antigravitycli/hooks.json` | `docs/hosts/antigravity-cli.md` |

### 5.2 Hook 事件与行为差异

| 事件 | Cursor | Codex | Claude Code |
|------|--------|-------|-------------|
| SessionStart | 轻量 `Repo:` 行 | 轻量 `source:` 行 | 类似 Cursor |
| UserPromptSubmit | pre-goal nudge | session key 硬前置 | review 提示 |
| PostToolUse | 证据采集 | 证据采集 | 证据采集 |
| Stop | review gate + closeout + SESSION_CLOSE_STYLE | review gate + closeout (可 `decision:block`) | review gate + closeout |
| beforeSubmit | paper adversarial/prose hook, subagent model inherit nudge | N/A | N/A |
| SessionEnd | hook-state 清理 | N/A | N/A |
| subagentStart | subagent 计数 | N/A | N/A |

关键差异：Codex 的 Stop 可以 `decision:block` 硬阻断；Cursor 的 Stop 是 `followup_message` 软提示。

### 5.3 Shell launcher

宿主 hook 配置不直接调用 `router-rs` 二进制，而是通过 shell launcher 脚本：

- Cursor: `configs/framework/cursor-router-rs-hook.sh`（二进制发现 + fail-closed/fail-open 分层）
- Claude: `configs/framework/claude-router-rs-hook.sh`
- Codex: `configs/framework/codex-router-rs-hook.sh`

二进制发现顺序：`ROUTER_RS_BIN` 环境变量 -> 仓库 `target/release/` -> `command -v router-rs`。缺失时关键门控事件 fail-closed，telemetry 事件 fail-open。

### 5.4 跨仓库接入

`scripts/cursor-bootstrap-framework.sh` 将 `skills/` 和 `AGENTS.md` 符号链接到目标仓库，并复制 hook 配置模板。支持 `--with-cursor-rules` 和 `--with-configs` 选项。

## 6. `configs/` 目录

### 6.1 `configs/framework/`

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

### 6.2 `configs/codex/` 和 `configs/framework/` 的关系

`configs/codex/` 包含 Codex 特定配置（如 aggregator 配置）。`configs/framework/` 是跨宿主的框架配置。

## 7. 测试与 CI

### 7.1 测试层次

| 测试文件 | 覆盖范围 | 运行方式 |
|----------|----------|----------|
| `tests/policy_contracts.rs` (111KB) | skill 路由契约、plugin catalog 闭集、manifest 一致性、research contract、深度 lane 等 | `cargo test --test policy_contracts` |
| `tests/host_integration.rs` (81KB) | 宿主集成测试、hook 输出格式、安装产物校验 | `cargo test --test host_integration` |
| `tests/documentation_contracts.rs` | 文档链接、命名约定、tracked markdown UTF-8 契约 | `cargo test --test documentation_contracts` |
| `tests/routing_eval_cases.json` | 路由评估用例（25KB JSON fixture） | 通过 `eval_route` 模块消费 |
| `tests/routing_route_fixtures.json` | 路由 fixture（10KB） | 路由引擎单测 |
| `tests/browser_mcp_scripts.rs` | Browser MCP 脚本测试 | `cargo test` |
| `tests/policy_cursor_rules_links.rs` | Cursor rules 链接校验 | `cargo test` |
| `tests/policy_markdown_links.rs` | Markdown 链接校验 | `cargo test` |
| `tests/tracked_markdown_utf8_contract.rs` | Tracked markdown UTF-8 契约 | `cargo test` |
| `tests/rust_cli_tools.rs` | Rust CLI 工具集成测试 | `cargo test` |
| `tests/autoresearch_cli.rs` | Autoresearch CLI 测试 | `cargo test` |
| `core/router-rs/tests/` | router-rs 单元测试（含 Claude Desktop hooks 测试） | `cargo test --manifest-path core/router-rs/Cargo.toml` |

### 7.2 Justfile 命令

```bash
just fmt           # cargo fmt
just clippy        # cargo clippy -D warnings
just test          # router-rs 测试
just test-all      # 全量测试（router-rs + antigravity + policy_contracts + host_integration）
just validate-skills  # skill 路由校验
just compile-skills   # skill 路由刷新
just doctor        # 框架健康检查
just ci            # validate-skills + test-all
```

### 7.3 CI 流水线

`.github/workflows/skill-ci.yml`：

- push/PR 触发
- 运行 `cargo test`（全量）
- 运行 `framework skills validate`
- 校验生成物漂移（metadata-only 模式）

`.github/workflows/evolution-audit.yml`：

- 定时触发
- 健康审计
- 同步 routing 产物
- 创建维护 issue

### 7.4 Schema Drift 检测

`router-rs schema-drift` 子命令组用于检测：

- hook 事件闭集（7 事件 Cursor、4 事件 Claude）是否与 contract 一致
- REQUIREMENTS/ROADMAP 标题格式是否符合约定
- 模板 parity（跨仓库 hooks.json 与 workspace-template 是否匹配）

### 7.5 生成物 Drift 检测

`framework host-integration generated-artifacts-status` 有两种模式：

- **metadata-only**（默认，`framework doctor` 使用）：只检查声明路径存在、forbidden marker、undeclared 路径
- **drift-gate**（全量，`framework maint update-one-shot` 使用）：在隔离 temp root 重跑 generator，byte/normalized 对比

## 8. 源码地图

```
.
+-- AGENTS.md                          # 跨宿主策略真源
+-- AGENTS_{CURSOR,CODEX,CLAUDE,ANTIGRAVITY}.md  # 宿主差异
+-- Cargo.toml                         # workspace 根
+-- Justfile                           # 开发命令
+-- cli/
|   +-- antigravity-cli/               # Antigravity CLI（独立二进制）
+-- core/
|   +-- antigravity/                   # 状态管理纯库（task_state、state_manager、rfv_loop）
|   +-- router-rs/                     # 核心控制平面二进制（hook、路由、门控、CLI）
|   +-- autoresearch-rs/               # 自动研究引擎
|   +-- evolution-rs/                  # 进化审计
+-- configs/
|   +-- framework/                     # 框架配置（registry、narrative、hook 脚本）
|   +-- codex/                         # Codex 特定配置
+-- skills/
|   +-- SKILL_ROUTING_RUNTIME.json     # 热路由真源
|   +-- SKILL_MANIFEST.json            # 冷热 manifest
|   +-- <skill-name>/SKILL.md          # 各 skill 定义
+-- rust_tools/                        # 独立 Rust 工具 crate（citation、image、pptx 等）
+-- tools/
|   +-- browser-mcp/                   # Browser MCP TypeScript 实现
+-- scripts/
|   +-- cursor-bootstrap-framework.sh  # 跨仓库接入脚本
|   +-- ci/                            # CI 脚本
+-- tests/                             # 集成测试（policy、host、routing eval）
+-- docs/                              # 契约文档
|   +-- harness_architecture.md        # 五层模型详细设计
|   +-- rust_contracts.md              # Rust 实现契约
|   +-- host_adapter_contract.md       # 多宿主适配契约
|   +-- hosts/                         # 各宿主接入手册
|   +-- references/                    # 扩展参考（operator surface、RFV、execution ladder）
+-- artifacts/                         # 运行产物（不入版本库）
+-- .cursor/                           # Cursor 工作区配置
+-- .claude/                           # Claude Code 配置
+-- .codex/                            # Codex 配置
+-- .github/workflows/                 # CI 流水线
```

## 9. 数据流概览

### 9.1 一次完整的用户请求

```
用户输入 -> 宿主捕获 -> shell launcher -> router-rs hook
  -> SessionStart: 注入轻量 Repo: 行
  -> UserPromptSubmit: session key 检查、pre-goal nudge
  -> [agent 执行，调用工具]
  -> PostToolUse: 证据采集到 EVIDENCE_INDEX
  -> [agent 继续执行]
  -> Stop: review gate 检查 -> closeout 检查 -> SESSION_CLOSE_STYLE 提示
```

### 9.2 Skill 路由流

```
用户意图
  -> router-rs route::routing 路由引擎
  -> 读取 SKILL_ROUTING_RUNTIME.json
  -> 匹配 trigger_hints + scoring
  -> 返回 skill_path
  -> 宿主读取对应 SKILL.md
```

### 9.3 Goal drive 流

```
用户调用 /implementx
  -> implementx SKILL.md 读取 WAVE_STATE.json
  -> 逐 wave 执行
  -> 每个 wave 产出写入 artifacts/current/<task_id>/
  -> 验证后 /verifyx 清理
```

### 9.4 证据流

```
L1 验证命令输出
  -> router-rs PostToolUse 采样/追加
  -> artifacts/current/<task_id>/EVIDENCE_INDEX.json
  -> closeout / review gate 消费
```
