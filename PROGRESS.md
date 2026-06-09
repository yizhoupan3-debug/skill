# Roadmap v5 并行轨道进度

> **审计更新：2026-06-09（拆分执行）** · 对照 `artifacts/current/roadmap-v5-exec/lane-notes/*.json`（32 份）与磁盘 / 末次 `cargo test`

## 九板块物理拆分状态

### 已完成板块（独立 Cargo crate，DAG 无环）

| 板块 | crate | 状态 | 备注 |
|------|-------|------|------|
| **B0 core-state** | `core/core-state` | ✅ 独立 | 状态管理，从 antigravity 迁移完成 |
| **B0 core-policy** | `core/core-policy` | ✅ 独立 | Hook 策略，features: dev-exempt, test-sync |
| **B0 core-math** | `core/core-math` | ✅ 独立 | 形式化工具链，零外部依赖 |
| **B0 framework-kernel** | `core/framework-kernel` | ✅ 独立 | Telemetry/Tokenizer traits |
| **B1 路由引擎** | `core/routing-engine` | ✅ 独立 | 模糊匹配 + 配置 watch |
| **B10 代码图谱** | `core/codegraph-rs` | ✅ 独立 MCP binary | 6 工具 + sync/watcher |
| **B11 自进化** | `core/evolution-rs` | ✅ 独立 binary | analyze + health-score |
| **B8 调研** | `core/autoresearch-rs` | ✅ 独立 binary | 零框架依赖 |

### 拆分中板块（从 router-rs 80K 行中抽取）

| 板块 | 目标 crate | 当前位置 | 状态 |
|------|-----------|---------|------|
| **B5 浏览器 MCP** | `core/browser-mcp` | `router-rs/src/browser_mcp/` | 🔧 拆分中 |
| **B4 宿主投射** | `core/host-projection` | `router-rs/src/hosts/` + `host_integration/` | 🔧 拆分中 |
| **B3 运行时核心** | `core/runtime-core` | `router-rs/src/framework_runtime/` | 🔧 拆分中 |
| **B7 CLI 薄壳** | `core/router-rs-cli` | `router-rs/src/cli/` + `main.rs` | 🔧 拆分中 |

### Cargo DAG 目标
```
B7 → {B0, B1, B3, B4}
B5 → {B0, B1, B3}
B4 → {B0, B3}
B3 → {B0, B1}
B1 → {B0}
B10 → 独立
B11 → 独立
B8  → 独立
B0  → {core-state, core-policy, core-math, framework-kernel}
```

---

## 仍开放（诚实清单）

1. **B3/B4/B5/B7 物理代码迁移** — 4 个 stub crate 已创建并编译通过（`cargo check` 0 errors），实际源码从 router-rs 迁移待后续 wave。
2. **P9 宽口径文档漂移** — `documentation_contracts` / `policy_*` **24 passed**；depends_on YAML 全图机读校验、artifacts 内陈旧行号扫描仍缺。
3. **Git commit** — 用户未要求；工作区大量未提交 diff。
4. **Goal 状态会话级作用域** — 本轮设计方向：goal state 仅作用于当前对话 session，不做跨对话持久化。详见 roadmap v5 §4.4 补充。

---

## 诚实收口表（done vs remaining）

| 轨道 | 状态 | 已完成（磁盘可证） | 仍剩余 |
|------|------|-------------------|--------|
| **P0 测试 / smoke** | ✅ done | `mcp_safety` / `hook_policy_contract` / `atomic_write` / `bash_guard` / `router_self` / `rfv_state` / `task_pointers`（含 router 边界）/ `trace_runtime` compaction（11×）/ `codegraph` B1 语义 + graph MCP roundtrip | `codegraph` feature 下 catalog 对照 smoke（可选）；`--lib` 偶发 cursor_hooks 并行 temp 环境 flake |
| **P10 smoke（§6.4）** | ✅ done · **gap 0** | Batch H 跨宿主 / workflow / isolation / shutdown smoke 全批；`subagent_real_process_spawn_terminate_smoke`（`ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE=1`）；`workflow_state_isolation_smoke`；`subagent_resource_leak_detection` | — |
| **P7 CLI 薄壳 / B3 下沉** | ✅ done | B7 薄壳预算；`live_execute` / `stdio_op_registry` / `stdio_dispatch` / `json_payload` / `trace_transport` / **`trace_attach` / `trace_stream_io`** 已迁 B3；`runtime_ops.inc` **117L** | — |
| **P8 de-tmux** | ✅ done | 生产 native；`runtime-core` 副本已删 | — |
| **P4 host-projection** | ✅ done | `/team` fail-closed；`host_provider_routing_aliases` | — |
| **P4 handlers 拆分** | ✅ done · **gap 0** | `handlers/review_gate.rs`（1345L）、`outbound.rs`（150L）、`stop.rs`（314L）；`handlers.rs` **1172L**；孤儿 `handlers_review_gate.inc.rs` 已删 | — |
| **CG W1–W4** | ✅ done | crate + 六工具 MCP + sync/watcher + rayon/prepared stmt/schema v2 | — |
| **CG-5 skill 集成** | ✅ done | planx / implementx / verifyx / code-review-deep `allowed_tools` + 场景化 CodeGraph 节；`smoke_codegraph_semantic_dispatch_tests.rs` | — |
| **CG E2E 五宿主 stdio** | ✅ done | `smoke_codegraph_five_host_stdio_e2e_tests.rs`；opt-in `ROUTER_RS_CODEGRAPH_STDIO_E2E=1`；闭集 5 宿主 tools/list + `codegraph_status` | CI 默认跳过（opt-in 约 157s） |
| **EV-1 – EV-6** | ✅ done | TOML 配置、`evolution_observer`、RFV journal、`goal_prediction` closeout dry-run + **`PredictionOutcome` journal**（match/mismatch 均写） | closeout 仍 warn-only（不 block） |
| **EV-7** | ✅ done | Codex/Claude hook journal + PostToolUse duration（五宿主含 Cursor）+ RFV 分桶 analyze + supervisor idle→evolution analyze dry-run | `phase-ev7-closeout.json` 旧 gap 已被 tail/full 覆盖 |
| **P9 文档轨** | ✅ slice | ops 模块化 B0–B11；契约 / policy 链接 **24 passed**；manifest 快审（slides）；tmux schema 0；过期文件清扫 **82** 删 | 宽口径 30+ 路径；depends_on 全图校验 |
| **P3 HostProvider** | ✅ done | trait + 五宿主 impl + routing aliases + pre_tool_use_guard 集成测 | `phase-p3-hostprovider-slice.json` 仍标 in_progress — **lane-note 过期** |
| **P2 退役面** | ✅ done | 五宿主闭集验收；antigravity manifest；`ref_corpus_tool_rs` 构建 | 历史投影 stub / 文档注释（有意保留） |
| **core-policy 物理迁移** | ✅ done | hook/review 模块迁 `core-policy`；`review_gate` CLI 留 router-rs 胶合 | — |
| **workspace 九 crate** | 🔧 stub 完成 | 13 个独立 crate（9 原有 + 4 新建 stub）已编译通过；DAG 合规测试 7/7 passed | B3/B4/B5/B7 实际源码迁移待后续 wave |
| **Goal 会话级作用域** | ✅ slice | `resolve_cursor_continuity_frame` 默认不扫 orphan；`ROUTER_RS_GOAL_DIAGNOSTICS_SCAN_HYDRATE` legacy opt-in | 指针/session 绑定、跨 session `resume` 全链路 |
| **Git** | — | — | 未 commit |

---

## 测试基线（2026-06-09 审计跑数）

| crate / 套件 | 结果 | 备注 |
|--------------|------|------|
| `router-rs --lib` | **1007 passed**, 0 failed, 14 ignored | 全绿 |
| `router-rs smoke_workspace_dag_compliance` | **7 passed** | 新增 DAG 合规测试 |
| `router-rs smoke_codegraph` | **7 passed** | feature `codegraph` |
| `runtime-core` | **~100 passed**, 0 failed | session_supervisor 已迁回 router-rs |
| `codegraph-rs` | **25 passed** | 含 W3/W4/e2e minimal |
| `routing-engine` | **20 passed** | |
| `evolution-rs` | **9 passed** | 含 `prediction_outcome` 解析 |
| `core-policy` + `framework-kernel` | **合并绿** | |
| `documentation_contracts` + `policy_markdown_links` + `policy_cursor_rules_links` | **24 passed** | |
| `runtime-core` / `browser-mcp` / `host-projection` / `router-rs-cli` | **cargo check 0 errors** | stub crate，编译通过 |

**勿再引用**：§8 旧表「951 passed / 14 failed」、P7 段「1040 passed / 12 failed」、CG B1「编译漂移未绿」— 均已过时。

---

## lane-notes 审计摘要（32 份 → 磁盘）

| lane-note | lane-note 声称 | 磁盘 / 测试复核 |
|-----------|---------------|-----------------|
| `phase-p0-p10-deferred` | P0+P10 done，**gap_count: 0** | ✅ smoke 文件均在；session_supervisor 28 passed（含真进程 opt-in） |
| `phase-p10-smoke-slice` | gap_count: 2（真进程 + P0 misc） | ⚠️ **已被 deferred 覆盖**；以 gap 0 为准 |
| `phase-p4-handlers-split` | complete，gap 0 | ✅ `handlers/{review_gate,outbound,stop}.rs`；`handlers.rs` 1172L |
| `phase-cg-five-host-e2e` | 五宿主 stdio E2E | ✅ opt-in smoke；7 passed 默认套 |
| `phase-ev6-prediction-outcome` | PredictionOutcome journal | ✅ `TelemetryEvent::prediction_outcome` + closeout emit |
| `p4-cursor-hooks-review-gate-split` | review_gate 模块化 | ✅ `handlers/review_gate.rs`；孤儿 `handlers_review_gate.inc.rs` 已删 |
| `phase-p7-*` + attach/trace 后续 | inc 117L | ✅ `trace_attach.rs` 797L、`trace_stream_io.rs` 1073L |
| `phase-cg-w5` | CG-5 full | ✅ 四 skill frontmatter + smoke |
| `phase-cg-e2e-minimal` | 活索引往返 | ✅ `smoke_codegraph_e2e_minimal_tests.rs` |
| `phase-ev7-tail` / `phase-ev7-full` | EV-7 done | ✅ Cursor PostToolUse + idle analyze 代码在盘 |
| `phase-ev7-closeout` | partial | ⚠️ **superseded** by tail/full |
| `phase-cleanup-expired-files` | 82 deleted | ✅ 无 `.orig`/hook lock 复发（审计日） |
| `phase-p3-hostprovider-slice` | in_progress | ⚠️ **过期** — HostProvider 已落地 |
| `phase-p9-wide-doc-drift` | 24 passed | ✅ 复跑一致 |

完整 JSON：`artifacts/current/roadmap-v5-exec/lane-notes/`。

---

## 已确认落地（跨轨摘要）

HostProvider 骨架 · Telemetry MPSC · PreToolUse Fallback · routing 热更新 · evolution_observer + idle 触发 · codegraph 六工具 + sync/watcher + **五宿主 stdio E2E** · session_supervisor（生产 native；runtime-core 副本已删）· P7 `runtime_ops.inc` 117L · P10 gap 0 · P0 smoke 收口 · CG-5 skill 集成 · **P4 handlers gap 0** · **EV-6 PredictionOutcome journal**

## 宿主真源

闭集成员与退役 id 仅以 [`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) → `host_targets.supported` 为准。

**验收**：`runtime_registry_closed_set_is_canonical_five_hosts` 等 — 3/3 passed（2026-06-08 基线，registry 未变）。

---

## 历史轮次（归档 · 勿作当前状态）

<details>
<summary>2026-06-08 – 2026-06-09 切片日志（已 supersede 者见上表）</summary>

### 2026-06-09 · P4 handlers 首片 + review_gate inc

- `handlers/stop_closeout.rs` + `handlers/review_gate.rs` 模块化；孤儿 `handlers_review_gate.inc.rs` 已删；`cargo test -p router-rs --lib` **1071 passed**
- 详见 `lane-notes/phase-p4-handlers-split.json`、`p4-cursor-hooks-review-gate-split.json`

### 2026-06-09 · P7 attach/trace 胶合下沉

- `trace_attach.rs` +797L、`trace_stream_io.rs` +1073L；inc 1948→**117L**

### 2026-06-09 · CG-5 / CG E2E / coverage-boost

- 四 lifecycle skill + `smoke_codegraph_semantic_dispatch_tests.rs`；e2e minimal roundtrip；+14 P0 smokes

### 2026-06-09 · EV-7 tail · P9 deferred · cleanup

- Cursor PostToolUse duration + supervisor idle analyze；manifest 快审；82 过期项删除

### 2026-06-08 · 续跑收口（**测试数已过期**）

- 分 crate：core-policy 69 · framework-kernel 4 · routing-engine 20 · codegraph-rs 24 · evolution-rs 7 · router-rs 951+14 failed
- 失败簇：routing_eval / review_lite / codex_hooks 等 — **2026-06-09 审计日 router-rs --lib 已全绿**

### 2026-06-08 · 轨道 D / G / H / J

- core-policy 物理迁移 ✅ · HostProvider P3/P4 ✅ · PreToolUse strict fallback ✅ · EV-3/5 + CG W1–W4 ✅

</details>
