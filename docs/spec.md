---
last_verified: "2026-06-09"
version: "unified-v5-full"
# 以下子文档是 spec.md 的延伸章节（Extension Chapters），各自保留聚焦内容。
# spec.md 为总览 + 索引 + 全局契约；子文档为子系统详细规约。
# 两者权威性等同，子文档在各自领域内为真源（详见各文档 frontmatter）。
extends:
  - docs/host_adapter_contract.md        # §7 宿主接入详表
  - docs/rust_contracts.md               # §8 路由与插件、§13 传输与持久化
  - docs/harness_architecture/           # §2 五层模型、§5 多 Agent 编排、§10 Hook 系统
  - docs/rust_contracts/                 # Rust 契约拆分详设
  - docs/framework_profile_contract.md   # §9.6 运维开关组合
  - docs/closeout_enforcement.md         # §12 Closeout 运维参考
  - docs/task_state_unified_resolve.md   # §3 Task state 设计
  - docs/rfv_loop_harness.md             # §12.2 RFV harness
  - docs/harness_policy_map.md           # 叙事裁判地图
---

# 框架统一规约 (Unified Framework Specification)

> 本文件是框架**总览规约**，覆盖架构总览、设计原则、crate 拓扑、测试契约与 schema 索引。
> 各子系统详细规约见下方 `extends` 列表中的延伸文档（各自在其领域内为真源）。
> 实施路线图见 [`artifacts/current/roadmap-v5-deep-review.md`](../artifacts/current/roadmap-v5-deep-review.md)。

---

## 目录

1. [架构总览](#1-架构总览)
2. [五层模型](#2-五层模型)
3. [Core Crates](#3-core-crates)
4. [运行期沙箱契约](#4-运行期沙箱契约)
5. [多 Agent 编排契约](#5-多-agent-编排契约)
6. [跨宿主统一矩阵](#6-跨宿主统一矩阵)
7. [宿主接入契约](#7-宿主接入契约)
8. [路由与插件契约](#8-路由与插件契约)
9. [运行时子系统](#9-运行时子系统)
10. [Hook 系统](#10-hook-系统)
11. [安全守卫](#11-安全守卫)
12. [Closeout 与生命周期](#12-closeout-与生命周期)
13. [传输与持久化](#13-传输与持久化)
14. [辅助模块](#14-辅助模块)
15. [可观测性](#15-可观测性)
16. [存储压缩](#16-存储压缩)
17. [测试契约](#17-测试契约)
18. [Schema 索引](#18-schema-索引)

---

## 1. 架构总览

### 1.1 Crate 拓扑

```
router-rs (95K LOC)          ← 主控制面
├── 32 个功能模块（§9-§14）
├── 704 个 #[test]
└── CLI 入口（clap）

B0 core crates（已从历史 framework-core 拆分）
├── core-state/（goal/rfv/evidence/pointers/state_manager）
├── core-policy/（hook_policy/review_gate）
├── core-math/（formal_toolchain）
├── framework-kernel/（telemetry/tokenizer traits）
└── 各 crate 独立 #[test]

codegraph-rs (2.3K LOC)      ← 代码图谱
├── types.rs（Node/Edge/FileRecord/ImpactReport）
├── parser/{common,rust,typescript,python,go}
├── db/{schema,node_ops,edge_ops,fts_ops,stats}
├── graph/{mod,build}
└── 0 #[test]（P0 缺口）

evolution-rs (851 LOC)       ← 技能进化审计
└── 2 #[test]

autoresearch-rs (6K LOC)     ← 研究工作区控制平面
└── 5 集成测试

rust_tools/ (9 crates, 16K LOC)
├── citation_tool_rs     ├── financial_data_rs    ├── gh_source_gate_rs
├── image_gen_rs         ├── image_process_rs     ├── ooxml_parser_rs
├── pptx_tool_rs         ├── pubmed_tool_rs       └── screenshot_rs
└── 各自独立二进制，无跨依赖
```

### 1.2 设计原则

| 原则 | 含义 |
|------|------|
| **单一权威真源** | `RUNTIME_REGISTRY.json` 为宿主闭集唯一权威 |
| **L4/L5 解耦** | 宿主差异仅存于 L4 适配壳 |
| **二元编排** | 仅 `subagent` + `workflow`；team 已废弃 |
| **纯 Rust 隔离** | PID + SQLite；tmux 已废弃 |
| **配置驱动接入** | 新宿主目标 3 文件 / 1-2 人天 |
| **Fail-closed** | 未知均默认拒绝 |

### 1.3 依赖关系约束

- B0 core crates 不依赖 `router-rs`
- host 特有逻辑禁止出现在 B0 core crates 中
- cli 层与 hosts 层单向依赖（目标：hosts 不引用 cli）

---

## 2. 五层模型

| 层 | 职责 | 允许 | 禁止 |
|----|------|------|------|
| **L0** | Skill 路由 | 路由信号、评分、准入 | 直接执行工具 |
| **L1** | Skill 契约 | verify_commands、拒因枚举 | 第二套连续性目录 |
| **L2** | 连续性工件 | artifacts/current/、EVIDENCE_INDEX schema | 与 L2 schema 冲突的并行真源 |
| **L3** | CLI 行为 | 门控、证据追加、closeout | 宿主 shell 复制 L3 决策 |
| **L4** | 宿主适配壳 | argv/stdin/超时/路径转发 | 长段策略 prose |
| **L5** | 宿主策略 | .mdc、AGENTS* 投影 | 与 L2 冲突的并行真源 |

---

## 3. Core Crates

### 3.1 B0 core crates — 领域模型（原 framework-core 已拆分）

**功能**：任务状态管理、Goal 驱动、RFV 循环、验证边界、步进账本、数学验证后端。

#### 3.1.1 state_manager/

| 子模块 | 功能 | 核心 API |
|--------|------|----------|
| **goal_state.rs** | Goal 生命周期（start/checkpoint/pause/resume/complete/block/clear） | `framework_goal_drive()`, `read_goal_state()`, `goal_state_requests_continuation()`, `deactivate_goal_for_conflict_with_rfv()` |
| **rfv_state.rs** | RFV 状态 + 外部研究验证 | `read_rfv_loop_state()`, `validate_external_research_structured/strict()`, `validate_adversarial_findings_structured()`, `validate_falsification_tests_structured()` |
| **task_pointers.rs** | active_task.json / focus_task.json 管理 | `read_active_task_id()`, `read_task_pointer_pair()`, `write_active_task_pointer()`, `neutralize_task_pointers_for_task()` |
| **evidence.rs** | EVIDENCE_INDEX.json + 可信度标注 | `annotate_evidence_row()`, `task_evidence_success_only_self_attested()`, `enrich_falsification_tests_with_execution()` |
| **verification_boundary.rs** | I8 验证边界 + I6 关键任务检测 | `default_verification_boundary()`, `is_key_task_goal()` |
| **hook_text_utils.rs** | 防欺骗、段落合并 | `scrub_spoof_host_followup_lines()`, `merge_hook_nudge_paragraph()` |

#### 3.1.2 task_state.rs — 统一读模型

- `resolve_task_view()` — 单次磁盘快照
- `resolve_continuity_frame()` — beforeSubmit/Stop 入口
- `hydrate_task_state_hybrid()` — TASK_STATE.json 聚合优先，回退物理文件
- `depth_compliance_aggregate()` — 跨 GOAL/RFV/EVIDENCE 深度评分（score 0-3）

**TaskControlMode**: `Idle` / `Goal` / `RfvLoop` / `Conflict`

#### 3.1.3 task_ledger.rs + step_ledger.rs

- **task_ledger**: TASK_LEDGER.jsonl 事务追加（L1 flock 保护），幂等去重
- **step_ledger**: STEP_LEDGER.jsonl 步进恢复账本，sha256 派生 idempotency key

#### 3.1.4 math_verify/ — 数学验证引擎

- `DimensionChecker::check_equal()` — 纯 Rust 量纲检查
- `FormalVerifier` — SymPy/Z3/Lean4 子进程调用
- `StepVerifier::verify()` — 步进证明验证
- `rollup_formal_depth_signal_from_goal()` — 形式化深度信号聚合

#### 3.1.5 utils/

| 工具 | 功能 |
|------|------|
| `atomic_write.rs` | temp + fsync + rename + fsync parent dir |
| `path_guard.rs` | 路径安全（禁止 ..、符号链接、根外访问） |
| `task_write_lock.rs` | 跨进程 advisory flock（`flock(2)`） |
| `read_bounded.rs` | 有界 UTF-8 前缀读取（hook 热路径优化） |
| `jsonl_maintenance.rs` | 损坏尾部截断 + 行数压缩 |

### 3.2 codegraph-rs — 代码知识图谱

**功能**：基于 tree-sitter 的代码图谱构建与查询，支持 Rust/TypeScript/JavaScript/Python/Go。入口：`core/codegraph-rs/src/lib.rs`；增量同步与 watcher：`graph/sync.rs`；MCP 薄壳分发：`core/router-rs/src/codegraph_mcp/mod.rs`。

| 模块 | 功能 | 核心 API |
|------|------|----------|
| `lib.rs` | crate 入口、`CodeGraphIndex` | `open`, `incremental_sync` |
| `types.rs` | 数据模型 | `Node`(13 种 NodeKind), `Edge`(6 种 EdgeKind), `ImpactReport` |
| `parser/` | 多语言符号/边提取 | `extract_from_file(path, language)` |
| `db/schema.rs` | SQLite schema（files + nodes + edges + FTS5） | `ensure_schema(conn)` |
| `db/node_ops.rs` | 节点 CRUD | `resolve_symbol()`, `find_nodes_by_qualified()` |
| `db/edge_ops.rs` | 边 CRUD + 图遍历 | `find_callers()`, `transitive_callers()`（递归 CTE） |
| `db/fts_ops.rs` | FTS5 全文搜索 | `search_symbols(query, kind, language)` |
| `graph/build.rs` | 全量索引构建 | `build_full_index(repo_root, conn)`（两遍插入 + 跨文件边解析） |
| `graph/sync.rs` | 增量 sync + filesystem watcher | `incremental_sync`, `spawn_watcher` |
| `graph/mod.rs` | 图查询 API | `impact_radius(symbol, max_depth)` |
| `mcp/mod.rs` | 六工具 MCP schema + dispatch | `codegraph_search`, `codegraph_callers`, … |

**CLI**: `index [--force]`, `status`, `query`, `callers`, `callees`, `impact`

### 3.3 evolution-rs — 技能进化审计

**功能**：从 JSONL 日志审计路由决策、检测模式冲突、生成健康评分、执行自动修复。

| 命令 | 功能 |
|------|------|
| `audit_journal` | TF-IDF 候选、边界碰撞、近似命中 |
| `generate_manifest` | 健康评分（动态 60% + 静态 40%） |
| `sync_feedback` | 同步 reroute/struggle 事件到反馈表 |
| `snapshot_skills` | 版本快照（带排他锁） |
| `heal_skills` | 自动剪枝零使用技能 |

### 3.4 autoresearch-rs — 研究工作区

**功能**：管理工作区生命周期（init → claim → hypothesis → run → reflect），维护 research-state.yaml。

**22 个子命令**：`init`, `status`, `next`, `resume`, `sync`, `draft-claims`, `plan-search`, `research-claim`, `research-all`, `gate-from-research`, `brief-first-claim`, `compare-claim`, `add-hypothesis`, `record-run`, `annotate-run`, `audit-reuse`, `reflect`, `set-novelty-gate` 等。

**外部 API**：Semantic Scholar, arXiv

### 3.5 rust_tools/ — 工具集

| Crate | 功能 | 核心命令 |
|-------|------|----------|
| `citation_tool_rs` | BibTeX 引用审计/lint/渲染 | `audit`, `claim-lint`, `render`(APA/IEEE/ACM/GB/T7714) |
| `financial_data_rs` | 金融数据获取（加密/美/港股） | `ohlcv`, `export`, `capital`, `validate` |
| `gh_source_gate_rs` | GitHub PR 门控 | `inspect-pr-checks`, `fetch-comments`, `doctor` |
| `image_gen_rs` | DALL-E 图像生成/编辑 | `generate`, `edit`, `generate-batch` |
| `image_process_rs` | 图像处理 | `resize`, `crop`, `convert`, `enhance`, `watermark` |
| `ooxml_parser_rs` | OOXML 解析（XLSX/DOCX/PPTX） | `xlsx`, `docx`, `render-xlsx`, `render-docx` |
| `pptx_tool_rs` | PPT 全功能（大纲/QA/Office 检查） | `init`, `outline`, `render`, `qa`, `office` |
| `pubmed_tool_rs` | PubMed API 客户端 | `search`, `fetch`, `fulltext` |
| `screenshot_rs` | 跨平台截图 | `capture`, `list_windows`, `capture_region` |

---

## 4. 运行期沙箱契约

### 4.1 生命周期状态机

```
created → warm → busy → draining → recycled → warm
                  ↓         ↓
                failed    failed
```

### 4.2 工具能力策略

类别：`read_only` · `workspace_mutating` · `networked` · `high_risk`

规则：按 Profile 声明 · 高风险独立 Profile · 重用保留边界 · deny-by-default

### 4.3 资源预算

维度：`cpu` · `memory` · `wall_clock` · `output_size`

超限 → `draining` + 持久失败原因。输出溢出不得包装为通用超时。

### 4.4 异步清理与隔离

- `draining` 时启动：释放临时文件/子进程/套接字/句柄（实现**异步清理**）
- 清理 100% 成功 → `recycled`；失败 → `failed`
- 单沙箱崩溃不得污染其他沙箱（进行**故障隔离**，确保 **recoverability boundary**）

---

## 5. 多 Agent 编排契约

> team 已废弃，tmux 已废弃。仅 `subagent` + `workflow`。

### 5.1 Subagent 生命周期

```
spawned → running → draining → completed
                    → failed
            → interrupted
```

### 5.2 隔离模型

| 维度 | 机制 |
|------|------|
| 进程 | `std::process::Command` detached + PID 文件 |
| 文件系统 | git worktree |
| 状态 | SQLite-backed（同 background_state） |
| 上下文 | `fork_context=false` |

### 5.3 Workflow 执行

| 模式 | 宿主 |
|------|------|
| `workflow_native`（JS 运行时） | claude-code |
| `workflow_supervisor`（Task 模拟） | 所有 |

### 5.4 Spawn Admission

允许：读重 · 独立假设 · 不阻塞 supervisor · disjoint 写入

拒绝原因：`small_task` · `shared_context_heavy` · `write_scope_overlap` · `next_step_blocked` · `verification_missing` · `token_overhead_dominates`

### 5.5 REVIEW_GATE 差异

> **清门真源（2026-06）**：`core-policy::review_gate_satisfied` — `independent_reviewer_seen`（`reviewer_lanes` + `fork_context=false`）或 override。**全宿主 Stop advisory-only**（不 `permission: deny` / `decision:block`）。详见 [`host_adapter_contract.md`](host_adapter_contract.md) §0.1。

| 能力 | claude-code | cursor | codex | opencode | antigravity |
|------|:-----------:|:------:|:-----:|:--------:|:-----------:|
| 可数深度 lane | `reviewer_lanes`（registry 共用闭集） | 同左 | 同左 | skill + `review-lanes/` | 同左 |
| spawn-first | ✅ | ✅ | ✅ | — | — |
| Stop 出站 | advisory nudge | advisory nudge | advisory nudge | MCP advisory | MCP advisory |
| my-light | suppress nudge | suppress nudge | suppress nudge | suppress nudge | suppress nudge |

---

## 6. 跨宿主统一矩阵

> 权威真源：`configs/framework/RUNTIME_REGISTRY.json`

### 6.1 宿主闭集

权威真源：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`（五 id 闭集）。

| 宿主 ID | install_tool | 运输模式 |
|---------|-------------|---------|
| `claude-code` | `claude` | `anthropic-claude-code` |
| `cursor` | `cursor` | `cursor-agent` |
| `codex` | `codex` | `native-codex` |
| `opencode` | `opencode` | `opencode-cli` |
| `antigravity` | `antigravity` | `mcp-stdio` |

> **退役 id** 不在闭集内；仅保留 stub 重定向页，见 [`MIGRATION.md`](../MIGRATION.md) 与 [`docs/hosts/`](hosts/) 下 `status: retired` 页。

### 6.2 Hook 事件矩阵

| 事件 | claude-code | cursor | codex | opencode | antigravity |
|------|:-----------:|:------:|:-----:|:--------:|:---------------:|
| PreToolUse | ✅ core | — | ✅ | — | — |
| UserPromptSubmit | ✅ core | ✅ ¹ | ✅ | — | — |
| PostToolUse | ✅ core | ✅ | ✅ | — | — |
| Stop | ✅ core | ✅ | ✅ | — | — |
| SessionStart | optional | ✅ | ✅ | — | — |
| SubagentStart | optional | ✅ | ✅ ² | — | — |
| SubagentStop | optional | ✅ | ✅ ² | — | — |

¹ `beforeSubmitPrompt` 映射 · ² v0.133.0+

### 6.3 MCP 配置差异

| 宿主 | 顶层 key | transport | 配置文件 |
|------|----------|-----------|----------|
| claude-code | `mcpServers` | `local`/`stdio` | `.claude/settings.json` |
| cursor | `mcp_servers` | `stdio` | `.cursor/mcp.json` |
| opencode | `mcp` | 无 type 字段 | `opencode.json` |
| antigravity | `mcp` | `stdio` | `.gemini/settings.json` |

**§3.5 Schema Drift 三道闸**：写盘前 validate → 写盘后 readback → manifest 路径存在性

### 6.4 编译嵌入矩阵

| 宿主 | 嵌入内容 | 机制 |
|------|----------|------|
| claude-code | settings.json 模板 | `host_integration/projection` |
| cursor | hooks.json + .mdc | `host_integration/projection` |
| codex | AGENTS.md + AGENTS_CODEX.md | `policy_embed.rs` |
| opencode | opencode.json 投影 | `host_integration/projection` |
| antigravity | .gemini/settings.json | `host_integration/projection` |

---

## 7. 宿主接入契约

### 7.1 目标路径（3 文件）

| # | 文件 | 操作 |
|---|------|------|
| 1 | `configs/framework/RUNTIME_REGISTRY.json` | 注册宿主 id + 元数据 |
| 2 | `core/runtime-core/src/hosts/<host>_hook_host.rs` | 实现 `HostHook` trait |
| 3 | `core/runtime-core/src/hosts/<host>_hooks/` | 事件 handler 目录 |

### 7.2 HostHook trait

```rust
pub trait HostHook {
    fn host_id(&self) -> &str;
    fn canonical_event(&self, raw: &str) -> Option<String>;
    fn critical_events(&self) -> &[&str];
    fn handle_pre_tool_use(&self, ctx: &HookContext) -> HookResult;
    fn handle_post_tool_use(&self, ctx: &HookContext) -> HookResult;
    fn handle_stop(&self, ctx: &HookContext) -> HookResult;
    fn handle_user_prompt_submit(&self, ctx: &HookContext) -> HookResult;
    fn handle_custom_event(&self, event: &str, ctx: &HookContext) -> Option<HookResult> { None }
}
```

### 7.3 接入 Checklist

- [ ] `RUNTIME_REGISTRY.json` — host_targets.supported + metadata
- [ ] `framework_host_targets.rs` — 只读注册表，fail-closed
- [ ] `hosts/<host>_hooks/` — 事件 handler
- [ ] `cli/dispatch.rs` — 子命令分发
- [ ] `host_integration/projection/` — install/status/remove + 三道闸
- [ ] `host_entrypoint_sync.rs` — provider trait
- [ ] 测试 + Schema 校验

### 7.4 硬编码耦合盘点

| 位置 | 内容 | 目标 |
|------|------|------|
| `framework_maint/mod.rs` | refresh_host_projections 遍历 | → registry 驱动 |
| `session_supervisor/mod.rs` | Codex driver only | → registry 标记 |
| `mcp_common/host.rs` | hard_closeout 列表 | → registry 数据驱动 |

---

## 8. 路由与插件契约

### 8.1 Skill 注册

**真源**：`skills/SKILL_ROUTING_RUNTIME.json`（列索引格式）

- `load_records_from_runtime()` — 纯配置加载
- `SKILL_ROUTING_METADATA.json` — 运行时元数据补丁
- `filter_records_for_host()` — 按 host_platforms 过滤
- 可插拔性：**4.0/5**

### 8.2 路由评分

- `route_task()` → `score_route_candidate()`（815 行硬编码，可插拔性 **2.0/5**）
- `signals/`：5 个子模块（design_artifact/devtools/markers/orchestration/paper）
- `nl_route_adjustments.rs`：NL suppress/boost 调整

### 8.3 路由引擎模块

| 子模块 | 功能 |
|--------|------|
| `routing.rs` | 主路由逻辑 + manifest fallback |
| `scoring.rs` | 候选评分（boost/suppress/overlay） |
| `records.rs` | 记录加载 + 缓存（mtime-based OnceLock+RwLock） |
| `policy.rs` | 路由策略载荷 |
| `text.rs` | 文本规范化 + 分词 |
| `aliases.rs` | 框架 alias 检测 |

---

## 9. 运行时子系统

### 9.1 browser_mcp/ — 浏览器 MCP 集成

**功能**：基于 CDP 的 MCP 服务器，提供浏览器自动化、页面快照、网络监控和 Skill 路由。

- 核心：`run_browser_mcp_stdio_loop()` — JSON-RPC 2.0 over stdio
- 30+ MCP 工具：browser_open/click/fill/screenshot/get_state/network/tabs 等
- Session 管理：session_launch/inspect/terminate/mark_blocked/resume_due
- 依赖：`background_state`, `session_supervisor`, `tungstenite`, `reqwest`

### 9.2 background_state/ — 后台任务状态

**功能**：持久化后台作业状态存储（filesystem/sqlite/memory 三后端）。

- 状态机：`queued → running → completed/failed/interrupted`
- 支持 `retry_scheduled/retry_claimed/retry_exhausted`
- 过期回收：活跃 1h TTL，终态 24h TTL
- 入口：`handle_background_state_operation()`

### 9.3 session_supervisor/ — Worker 生命周期

**功能**：Worker 生命周期管理（launch/resume/terminate/mark_blocked/resume_due）。

- 驱动：codex/cursor/claude/antigravity（`driver.rs`）
- 原生进程驱动（P8 de-tmux 已接线 router-rs `session_supervisor/`；`runtime-core` 副本待删）
- 速率限制检测：正则模式匹配
- 入口：`handle_session_supervisor_operation()`

### 9.4 framework_runtime/ — 运行时行为

**功能**：运行时快照、契约摘要、workspace 初始化、doctor 检查、状态行构建。

| 子文件 | 功能 |
|--------|------|
| `runtime_view.rs` | 运行时视图 + 连续性分类 |
| `workspace_init.rs` | `router-rs init` |
| `framework_doctor.rs` | Doctor 健康检查 + 连续性审计 |
| `session_artifacts.rs` | 会话 artifact 写入 |
| `statusline.rs` | 状态行构建 |
| `prompt_compression.rs` | prompt 压缩策略 |
| `constants.rs` | schema version + authority |

- 快照生成：可以通过 `router-rs framework snapshot` 生成包含完整连续性视图的运行时快照只读模型。

### 9.5 framework_maint/ — 维护命令

**功能**：`router-rs framework maint ...` 维护子命令集。

子命令：`RefreshHostProjections` · `VerifyCursorHooks` · `VerifyCodexHooks` · `UpdateOneShot` · `UpdateAudit` · `CleanRustTargets` · `PrintLocalHomes` · `InstallCodexUserHooks` · `ContinuityAudit`

### 9.6 framework_profile/ — Profile 管理

**功能**：框架 Profile 编译、artifact 打包和控制平面契约描述符。

- `FrameworkProfileContract` — profile_id/capabilities/mcp_servers 等 20+ 字段
- `ProfileBundle` + `CapabilityBundle`
- 控制平面：`build_control_plane_contract_descriptors()`

---

## 10. Hook 系统

### 10.1 hook_common/ — 共享 Hook 工具

**功能**：跨宿主 hook 共享逻辑（路径守卫、证据追加、信号检测、lane 归一化）。

- 28 个 pub fn
- 子模块：`path_guard.rs`, `evidence.rs`, `lane_normalize.rs`, `hook_observation_rules.rs`

### 10.2 hook_policy/ — Hook 策略

**功能**：Bash 命令危险分类、MCP 工具安全检测、受保护路径识别。

| 子模块 | 功能 | 核心 API |
|--------|------|----------|
| `bash_guard.rs` | Bash 命令分析（正则模式匹配） | `dangerous_bash_reason()` |
| `mcp_safety.rs` | MCP 工具安全 | `dangerous_mcp_tool_reason()` |
| `evaluate.rs` | 统一策略评估 | `evaluate_hook_policy()` |
| `contract.rs` | 契约 JSON | `hook_policy_contract()` |

### 10.3 review/ — Review 引擎

**功能**：Review gate 执行、异构对抗审稿路由、输出格式 lint。

| 子模块 | 功能 |
|--------|------|
| `engine.rs` | Review gate 核心（Strict/Lite 模式） |
| `heterogeneous.rs` | 异构对抗审稿（ModelFamily 检测 + 跨族验证） |
| `output_lint.rs` | Review 输出格式 lint |
| `routing_signals.rs` | Review 路由信号 |

**ModelFamily**: Claude/Gpt/Gemini/Llama/Mistral/Deepseek

---

## 11. 安全守卫

### 11.1 web_fetch_guard.rs — SSRF 防护

**功能**：限制 web_fetch 仅访问公网，阻止 loopback/CGNAT/link-local/私有网段。

- `validate_web_fetch_url()` — URL 校验
- `validate_and_resolve_web_fetch_url()` — DNS 解析（防 TOCTOU rebinding）
- 阻止：`.localhost`、`.local`、`.internal`、loopback、CGNAT（100.64/10）

### 11.2 mcp_pre_guard.rs — MCP 前置守卫

**功能**：MCP `tools/call` 前置安全检查，panic 时降级为 block。

- 工具安全检查 + 受保护路径检测
- 依赖：`hook_common::path_guard`、`hook_policy::dangerous_mcp_tool_reason`

---

## 12. Closeout 与生命周期

### 12.1 Goal State

**真源**：`artifacts/current/<task_id>/GOAL_STATE.json`

操作：`start` · `checkpoint` · `pause` · `resume` · `complete` · `block` · `clear`

契约：`drive_until_done=true` 时强制 non_goals + done_when≥2 + validation_commands

### 12.2 RFV Loop

**真源**：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`

操作：`start`（upsert）· `append_round`

关闭门控：`verify_pass` · `min_depth_score` · `external_research_strict`

### 12.3 closeout_enforcement.rs

**功能**：closeout 门控强制执行与管理。

- `evaluate_closeout_record_value()` — 评估 closeout 记录
- `summary_claims_completion()` — 摘要是否声称完成
- **my-light**: advisory；**非 my-light**: closeout fail-closed
- `closeout_gate` — 门控定义及拦截逻辑，用于验证是否满足 closeout 状态
- `closeout_record_write` — 写入 closeout 记录与断言结果

### 12.4 ship_readiness.rs

**功能**：Goal/Stop followup 评估 — 磁盘检查就绪度 + followup 提示。

- `evaluate_goal_readiness_from_disk()` — contract/progress/verification 三元组
- `goal_stop_followup_line()` — Stop followup 文案

### 12.5 连续性锚点

| 锚点 | 路径 |
|------|------|
| EVIDENCE_INDEX | `artifacts/current/EVIDENCE_INDEX.json` |
| NEXT_ACTIONS | `artifacts/current/NEXT_ACTIONS.json` |
| SESSION_SUMMARY | `artifacts/current/SESSION_SUMMARY.md` |
| TRACE_METADATA | `artifacts/current/TRACE_METADATA.json` |
| GOAL_STATE | `artifacts/current/<task_id>/GOAL_STATE.json` |
| RFV_LOOP_STATE | `artifacts/current/<task_id>/RFV_LOOP_STATE.json` |
| TASK_LEDGER | `artifacts/current/<task_id>/TASK_LEDGER.jsonl` |
| STEP_LEDGER | `artifacts/current/<task_id>/STEP_LEDGER.jsonl` |

### 12.6 生命周期 Profile

| Profile | REVIEW_GATE | AG_FOLLOWUP | closeout |
|---------|:-----------:|:-----------:|:--------:|
| my-light | advisory（suppress nudge） | advisory | advisory |
| full | advisory（nudge） | advisory | fail-closed |

---

## 13. 传输与持久化

### 13.1 runtime_storage/ — 运行时存储

**功能**：filesystem/sqlite/memory 三后端统一抽象。

- 操作：read/write/append/exists/delete/stat
- 路径级文件锁（`acquire_runtime_path_lock`）
- SQLite WAL 模式

### 13.2 runtime_registry/ — 运行时注册表

**功能**：磁盘优先 `RUNTIME_REGISTRY.json` 加载器。

- `HookRegistryRepoGuard` — RAII 守卫
- 缓存：mtime-based OnceLock

### 13.3 stdio_transport.rs — Stdio 传输层

**功能**：并发 JSON-over-stdio 传输层。

- Worker 池：默认 8，最大 32
- 超时：默认 30s，最大 3600s
- in-flight 超时 + 批量响应刷新
- 支持 stdio `execute` operation 处理机制

### 13.4 host_entrypoint_sync.rs — 入口同步

**功能**：通用 sync engine + Codex provider。

- `full_sync`（root）vs `partial_sync`（worktree）
- `HostProjectionAdapter` — 薄 adapter 表

### 13.5 host_integration/ — 安装/投影

- `install_<host>_projection` — 投影写盘
- `remove_<host>_projection` — 投影移除
- `<host>_projection_status` — 投影状态查询
- 三道闸：写盘前 validate + 写盘后 readback + manifest 路径存在性

---

## 14. 辅助模块

### 14.1 paper_adversarial_hook.rs — 论文对抗审稿

- opt-in，per-host 环境变量控制
- 文案真源：`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`（`include_str!`）

### 14.2 paper_prose_hook.rs — 论文润色

- 默认开启，per-host 环境变量关闭
- 合并 prose quality chain 短段

### 14.3 harness_operator_nudges.rs — 运维提示

- RFV/Goal drive 的运维提示行注入
- 真源：`configs/framework/HARNESS_OPERATOR_NUDGES.json`

### 14.4 harness_context_signals.rs — 上下文信号

- 数学/形式化验证上下文的启发式检测
- 为 harness 运维提示提供信号

### 14.5 harness_contract.rs — Harness 契约

- 失败分类学（10 类：route_miss/owner_drift/context_rot 等）
- Skill 契约 lint（frontmatter 完整性）

### 14.6 formal_toolchain.rs — 形式化验证检测

- ASCII 小写子串启发式检测形式化证明工具（SymPy/Z3/Lean/Coq/Isabelle/Agda）

### 14.7 schema_drift.rs — Schema 漂移检测

- 任务/harness schema 漂移基线捕获与对比验证
- 确保 hook 事件集、artifact 结构、契约版本不漂移

### 14.8 execution_contract.rs — 执行契约

- 执行内核元数据 + 契约包 + 实时响应序列化契约
- 纯数据构建（无模块依赖），43 个测试

### 14.9 router_rs_observation.rs — 观测

- Hook 出站 JSON 的结构化观测载荷注入
- 门控分类、相关性提取、阻断判定

### 14.10 session_call_tracker.rs — 会话调用追踪

- 工具调用和 token 使用追踪
- 异常检测：单工具上限、总调用上限、skill_route 首次检测
- 持久化：`artifacts/current/SESSION_CALL_TRACKER.json`

### 14.11 html_to_markdown.rs + content_extract.rs

- `html_to_markdown()` — 纯 Rust HTML → Markdown
- `extract_readable_content()` — readability 替代方案

### 14.12 trace_runtime.rs — 追踪运行时

- Trace 事件记录、流式输出、压缩、delta 快照和重放
- schema：`runtime-trace-v2`

### 14.13 runtime_envelope_ids.rs — 常量集中

- 30+ schema_version/authority 常量
- 资源限制：`DEFAULT_MAX_CONCURRENT_SUBAGENTS=8`, `MAX=24`

### 14.14 router_self.rs — 自身管理

- `router-rs self install|clean` — 全局二进制安装/清理
- macOS 自动 ad-hoc 签名刷新

### 14.15 router_env_flags.rs — 环境变量开关

- 30+ `ROUTER_RS_*` 环境变量 helper
- 通用 token：`1`/`true`/`yes`/`on` = enabled；`0`/`false`/`off`/`no` = disabled

---

## 15. 可观测性

> aspirational — 部分已实现（trace_runtime），部分计划中。

### 15.1 JSONL ↔ OTel 映射

| JSONL 键 | OTel 属性 | 信号 |
|----------|-----------|------|
| `ts` | `time_unix_nano` | span/metric/log |
| `event_id` | `runtime.event.id` | span/log |
| `kind` | `runtime.kind` | span/log |
| `stage` | `runtime.stage` | span/metric/log |
| `status` | `runtime.status` | span/metric/log |
| `job_id` | `runtime.job_id` | span/metric/log |
| `session_id` | `runtime.session_id` | span/metric/log |

### 15.2 核心指标

| 指标 | 类型 | 状态 |
|------|------|------|
| `runtime.route_mismatch_total` | Counter | aspirational |
| `runtime.sandbox_timeout_total` | Counter | aspirational |
| `runtime.subagent_spawn_total` | Counter | aspirational |
| `runtime.workflow_phase_duration_ms` | Histogram | aspirational |

---

## 16. 存储压缩

> aspirational — 运行期契约冻结，实现待定。

### 16.1 快照与增量

- **SnapshotCheckpoint**：每代一个快照（schema_version, generation, snapshot_id, state_digest, delta_cursor）
- **增量日志**：Latest Snapshot + Monotonic Generation-local Deltas
- **生成物分离**：大对象写入 Artifact Refs

### 16.2 世代滚动

- 新世代继承最小必要状态
- 旧世代保持可读
- 世代号单调递增
- 回放不能要求扫描全量历史流

---

## 17. 测试契约

### 17.1 覆盖率现状（2026-06-08）

| Crate | LOC | #[test] | 评级 |
|-------|-----|---------|------|
| router-rs | 95K | ~1,577 | B+ |
| B0 core crates（core-state 等） | ~10K 合计 | ~161 | B |
| codegraph-rs | 2.3K | 25 | C |
| evolution-rs | 851 | 2 | D |
| autoresearch-rs | 6K | 8 | D |
| rust_tools (9) | 16K | ~102 | C |
| **合计** | **130K** | **~1,850** | |

### 17.2 router-rs 子模块覆盖

| 模块 | #[test] | | 模块 | #[test] |
|------|---------|---|------|---------|
| runtime_storage | 117 | | hook_policy | 97 |
| route | 66 | | session_supervisor | 54 |
| review | 49 | | execution_contract | 43 |
| web_fetch_guard | 31 | | framework_runtime | 26 |
| rfv_loop | 26 | | framework_profile | 12 |
| paper_adversarial_hook | 14 | | harness_context_signals | 11 |
| session_call_tracker | 10 | | paper_prose_hook | 10 |
| router_env_flags | 9 | | background_state | 9 |
| router_rs_observation | 8 | | stdio_transport | 7 |
| harness_operator_nudges | 7 | | schema_drift | 6 |
| mcp_pre_guard | 6 | | browser_mcp | 5 |
| runtime_registry | 5 | | html_to_markdown | 4 |
| ship_readiness | 3 | | formal_toolchain | 3 |
| harness_contract | 2 | | framework_maint | 2 |
| **trace_runtime** | **0** | | **router_self** | **0** |
| **runtime_envelope_ids** | **0** | | **content_extract** | **1** |

### 17.3 P0 测试缺口

| 文件 | pub fn | 说明 |
|------|--------|------|
| `codegraph-rs` (全部) | 41 | 代码图核心逻辑 |
| `router_self.rs` | 14 | 二进制安装/验证/分发 |
| `state_manager/rfv_state.rs` | 10 | RFV 状态验证 |
| `state_manager/task_pointers.rs` | 7 | 任务指针读写同步 |
| `hook_policy/bash_guard.rs` | 6 | Bash 命令安全分类 |
| `trace_runtime.rs` | 0 | 追踪运行时（855 行零测试） |

### 17.4 Smoke Test 契约

**P0**：Subagent 生命周期（5）· 关闭流程（3）· Workflow 稳定性（4）

**P1**：跨宿主一致性（3）· 资源隔离（3）

### 17.5 Schema 校验测试

- 每个宿主投影 MCP key 名：happy-path + sad-path
- 测试夹具与生产共享 `make_<host>_payload()` 工厂函数

---

## 18. Schema 索引

| Schema | 版本 | 来源模块 |
|--------|------|----------|
| `runtime-sandbox-contract-v1` | §4 | 沙箱生命周期 |
| `multi-agent-orchestration-contract-v1` | §5 | 编排单元 |
| `framework-runtime-registry-v1` | §6 | 注册表 |
| `router-rs-route-decision-v1` | §8 | 路由决策 |
| `router-rs-execute-response-v1` | §9 | 执行响应 |
| `router-rs-hook-policy-v1` | §10 | Hook 策略 |
| `router-rs-harness-contract-v1` | §14 | Harness 契约 |
| `runtime-trace-v2` | §15 | 追踪事件 |
| `router-rs-background-state-store-v1` | §9 | 后台状态 |
| `router-rs-session-supervisor-response-v1` | §9 | Session Supervisor |
| `router-rs-rfv-loop-v1` | §12 | RFV 循环 |
| `router-rs-hook-observation-v1` | §14 | Hook 观测 |
| `schema-drift-baseline-v1` | §14 | Schema 漂移 |

---

## 契约漂移规则

本规约中的机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。

涉及上述配置规则的代码变更，**必须以"文档先行"形式首先修改本文件**，然后进行 Rust 实现与测试回归。

禁止：
- 从一个宿主复制 adapter 模板到另一个宿主而不改 key 名
- 测试夹具用"预期 bug 形态"反向锁死 bug
- 新增宿主/模块时不更新本文件
