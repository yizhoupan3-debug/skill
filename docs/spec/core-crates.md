---
parent: docs/spec.md
version: unified-v7
---

## 3. Core Crates

### 3.1 B0 core crates — 领域模型（原 framework-core 已拆分）

**功能**：任务状态管理、Goal 驱动、RFV 循环、步进账本、Goal 预测。

#### 3.1.1 state_manager/

| 子模块 | 功能 | 核心 API |
|--------|------|----------|
| **mod.rs** (goal_state) | Goal 生命周期（start/checkpoint/pause/resume/complete/block/clear） + 证据摘要 | `framework_goal_drive()`, `read_goal_state()`, `goal_state_requests_continuation()`, `task_evidence_success_only_self_attested()` |
| **rfv_ops.rs** | RFV 状态管理 + 冲突消解 | `read_rfv_loop_state()`, `deactivate_rfv_for_conflict_with_goal_drive()`, `deactivate_goal_for_conflict_with_rfv()` |
| **pointer_ops.rs** | active_task.json / focus_task.json 管理 | `read_active_task_id()`, `read_task_pointer_pair()`, `write_active_task_pointer()`, `neutralize_task_pointers_for_task()` |
| **scrub_ops.rs** | 防欺骗、段落合并、hook 输出清理 | `scrub_spoof_host_followup_lines()`, `merge_hook_nudge_paragraph()`, `scrub_followup_fields_in_hook_output()` |
| **validation.rs** | 外部研究验证 | `validate_external_research_structured()`, `validate_external_research_strict()`, `source_traceable_heuristic()` |

#### 3.1.2 task_state.rs — 统一读模型

- `resolve_task_view()` — 单次磁盘快照
- `resolve_cursor_continuity_frame()` — beforeSubmit/Stop 入口
- `hydrate_task_state_hybrid()` — TASK_STATE.json 聚合优先，回退物理文件
- `depth_compliance_aggregate()` — 跨 GOAL/RFV/EVIDENCE 深度评分（score 0-3）

**TaskControlMode**: `Idle` / `GoalDrive` / `RfvLoop` / `Conflict { reason: String }`

#### 3.1.3 task_ledger.rs + step_ledger.rs

- **task_ledger**: TASK_LEDGER.jsonl 事务追加（L1 flock 保护），幂等去重
- **step_ledger**: STEP_LEDGER.jsonl 步进恢复账本，sha256 派生 idempotency key

#### 3.1.4 goal_prediction.rs — Goal 状态预测

- `GoalStatePrediction` — 基于当前状态预测 Goal 最终结果
- `PredictionVerification` — 预测验证

#### 3.1.5 utils/

| 工具 | 功能 |
|------|------|
| `atomic_write.rs` | temp + fsync + rename + fsync parent dir |
| `path_guard.rs` | 路径安全（禁止 ..、符号链接、根外访问） |
| `task_write_lock.rs` | 跨进程 advisory flock（`flock(2)`） |
| `read_bounded.rs` | 有界 UTF-8 前缀读取（hook 热路径优化） |
| `jsonl_maintenance.rs` | 损坏尾部截断 + 行数压缩 |

### 3.2 loop-engine — 循环引擎

**功能**：loop-auto profile 的状态机实现，管理 discover→preflight→dispatch→verify→report 全流程。

| 模块 | 功能 |
|------|------|
| `runner.rs` | 循环主循环、phase 转换、kill signal 轮询 |
| `types.rs` | `LoopPhase` 枚举（9 阶段）、`LoopRunState`、serde roundtrip |
| `safety.rs` | 基于 scope 的 L1/L2/L3 安全等级分配、glob 模式匹配 |
| `kill_switch.rs` | 文件级 kill signal（`.loop-kill/<loop-id>`）、stale 检测 |
| `closeout.rs` | closeout 验证（task_id/summary/verification_status/changed_files） |
| `dispatcher.rs` | opencode CLI 子进程派发、5s kill-poll loop、600s 超时 |
| `report.rs` | `LOOP_REPORT.md` 渲染（summary + per-action + unconsumed findings） |
| `state.rs` | 原子写入 `LOOP_RUN_STATE.json`、run history、circuit breaker |
| `lib.rs` | crate 入口、公共 API re-export |

**状态**：v8.0 scope 内已实质完成（44 测试通过），budget enforcement 为 soft no-op，SubagentExecutor trait 延迟到 v8.2。

### 3.3 tools/codegraph-rs — 代码知识图谱

**功能**：基于 tree-sitter 的代码图谱构建与查询，支持 Rust/TypeScript/JavaScript/Python/Go。位于 `tools/codegraph-rs/`（v7 从 `core/` 迁出）。入口：`tools/codegraph-rs/src/lib.rs`；增量同步与 watcher：`graph/sync.rs`；MCP 薄壳分发：`core/runtime-core/src/codegraph_mcp/mod.rs`。

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
| `mcp/mod.rs` | 八工具 MCP schema + dispatch | `codegraph_search`, `codegraph_callers`, …, `codegraph_goto_definition` |

**CLI**: `index [--force]`, `status`, `query`, `callers`, `callees`, `impact`

### 3.3 tools/evolution-rs — 技能进化审计

**功能**：从 JSONL 日志审计路由决策、检测模式冲突、生成健康评分、执行自动修复。

| 命令 | 功能 |
|------|------|
| `audit_journal` | TF-IDF 候选、边界碰撞、近似命中 |
| `generate_manifest` | 健康评分（动态 60% + 静态 40%） |
| `sync_feedback` | 同步 reroute/struggle 事件到反馈表 |
| `snapshot_skills` | 版本快照（带排他锁） |
| `heal_skills` | 自动剪枝零使用技能 |

### 3.4 core/research-harness — 研究工作区（含 autoresearch CLI）

**功能**：管理工作区生命周期（init → claim → hypothesis → run → reflect），维护 research-state.yaml。

**22 个子命令**：`init`, `status`, `next`, `resume`, `sync`, `draft-claims`, `plan-search`, `research-claim`, `research-all`, `gate-from-research`, `brief-first-claim`, `compare-claim`, `add-hypothesis`, `record-run`, `annotate-run`, `audit-reuse`, `reflect`, `set-novelty-gate` 等。

**外部 API**：Semantic Scholar, arXiv

### 3.5 rust_tools/ — 工具集

| Crate | 功能 | 核心命令 |
|-------|------|----------|
| `citation_tool_rs` | BibTeX 引用审计/lint/渲染 | `audit`, `claim-lint`, `render`(APA/IEEE/ACM/GB/T7714) | MCP: `mcp-citation` |
| `financial_data_rs` | 金融数据获取（加密/美/港股） | `ohlcv`, `export`, `capital`, `validate` | MCP: `mcp-financial-data` |
| `gh_source_gate_rs` | GitHub PR 门控 | `inspect-pr-checks`, `fetch-comments`, `doctor` | MCP: `mcp-gh-source-gate` |
| `ooxml_parser_rs` | OOXML 解析（XLSX/DOCX/PPTX） | `xlsx`, `docx`, `render-xlsx`, `render-docx` | MCP: `mcp-ooxml` |
| `pdf_tool_rs` | PDF 文本提取 | `pdf_read`, `pdf_info` | MCP: `mcp-pdf` |
| `pptx_tool_rs` | PPT 全功能（大纲/QA/Office 检查） | `init`, `outline`, `render`, `qa`, `office` | MCP: `mcp-pptx` |

> **已归档** (v6 之前)：`image_gen_rs`, `image_process_rs`, `pubmed_tool_rs`, `ref_corpus_tool_rs`, `screenshot_rs` — 无下游依赖，从 workspace 移除。

### 3.6 模块解耦架构（合并自 `docs/architecture/module-decoupling.md`）

#### 层状依赖图

```
Layer 0 (leaf):  core-state  framework-kernel  routing-engine
                 tools/codegraph-rs  core/research-harness  tools/evolution-rs
                 trace-runtime       runtime-storage
Layer 1:         core-policy
Layer 2:         host-projection
Layer 3:         runtime-core (facade) → framework-runtime  session-supervisor
                 runtime-core → browser-mcp
Layer 4:         router-rs
```

无循环依赖。`runtime-core` 是最大的耦合热点。

#### 拆分计划（v7）— 执行状态

| 新 crate | 路径 | 职责 | 预计行数 | 提取来源 |
|----------|------|------|---------|---------|
| `runtime-storage` | `core/runtime-storage/` | 状态持久化、文件锁、atomic write、后台任务状态 | ~8K | `runtime_storage/`, `background_state/` |
| `framework-runtime` | `core/framework-runtime/` | 框架运行时核心循环、execution contract、pre_tool_use_guard、closeout enforcement、trace I/O | ~5K | `closeout_enforcement.rs`, `execution_contract.rs`, `pre_tool_use_guard.rs`, `runtime_view.rs`, `trace_stream_io.rs`, `trace_attach.rs`, `trace_transport.rs`, `live_execute.rs`, `sandbox_control.rs`, `evolution_observer.rs` |
| `session-supervisor` | `core/session-supervisor/` | Worker 管理、session 生命周期、evolution_idle | ~5K | `session_supervisor/`, `harness_operator_nudges.rs` |
| `trace-runtime` | `core/trace-runtime/` | 事件追踪、observation、journal 聚合入口；共享 trace helper（`trace_event_object`, `hydrate_trace_event`, `sha256_hex` 等） | ~1K | `trace_runtime.rs` |
| `telemetry-types` | `core/telemetry-types/` | `TelemetryEvent` / `PredictionOutcomeCheck` 共享类型定义（单源，framework-kernel 和 evolution-rs 共用） | ~80 | 新建 |

#### 依赖规则

1. 下层不得依赖上层。
2. `runtime-core`（facade）可为向后兼容 re-export 已提取 crate 的内容。
3. `browser-mcp` 只通过 `runtime-core` 获取共享状态类型。
4. 任何 crate 不得依赖 3 个以上 workspace 内的 path dependencies。

---

