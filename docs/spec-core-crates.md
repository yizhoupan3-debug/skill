---
parent: docs/spec.md
version: unified-v7
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

**功能**：基于 tree-sitter 的代码图谱构建与查询，支持 Rust/TypeScript/JavaScript/Python/Go。入口：`tools/codegraph-rs/src/lib.rs`；增量同步与 watcher：`graph/sync.rs`；MCP 薄壳分发：`core/runtime-core/src/codegraph_mcp/mod.rs`。

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
| `citation_tool_rs` | BibTeX 引用审计/lint/渲染 | `audit`, `claim-lint`, `render`(APA/IEEE/ACM/GB/T7714) | MCP: `mcp-citation` |
| `financial_data_rs` | 金融数据获取（加密/美/港股） | `ohlcv`, `export`, `capital`, `validate` | MCP: `mcp-financial-data` |
| `gh_source_gate_rs` | GitHub PR 门控 | `inspect-pr-checks`, `fetch-comments`, `doctor` | MCP: `mcp-gh-source-gate` |
| `ooxml_parser_rs` | OOXML 解析（XLSX/DOCX/PPTX） | `xlsx`, `docx`, `render-xlsx`, `render-docx` | MCP: `mcp-ooxml` |
| `pdf_tool_rs` | PDF 文本提取 | `pdf_read`, `pdf_info` | MCP: `mcp-pdf` |
| `pptx_tool_rs` | PPT 全功能（大纲/QA/Office 检查） | `init`, `outline`, `render`, `qa`, `office` | MCP: `mcp-pptx` |

> **已归档** (v6 之前)：`image_gen_rs`, `image_process_rs`, `pubmed_tool_rs`, `ref_corpus_tool_rs`, `screenshot_rs` — 无下游依赖，从 workspace 移除。

---

