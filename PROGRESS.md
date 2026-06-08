# Roadmap v5 并行轨道进度

> **审计更新：2026-06-09（续跑）** · 对照 `artifacts/current/roadmap-v5-exec/lane-notes/*.json`（32 份）与磁盘 / 末次 `cargo test`

## 仍开放（诚实清单）

1. **`core-state` 物理目录** — 现仍 `antigravity` 别名；独立 crate 迁移为 roadmap blocker，未强行大迁。
2. **九板块独立 crate** — 未达成（§2.0–2.1 大项，15–20d 量级）。
3. **P9 宽口径文档漂移** — `documentation_contracts` / `policy_*` **24 passed**；depends_on YAML 全图机读校验、artifacts 内陈旧行号扫描仍缺。
4. **Git commit** — 用户未要求；工作区大量未提交 diff。

---

## 诚实收口表（done vs remaining）

| 轨道 | 状态 | 已完成（磁盘可证） | 仍剩余 |
|------|------|-------------------|--------|
| **P0 测试 / smoke** | ✅ done | `mcp_safety` / `hook_policy_contract` / `atomic_write` / `bash_guard` / `router_self` / `rfv_state` / `task_pointers`（含 router 边界）/ `trace_runtime` compaction（11×）/ `codegraph` B1 语义 + graph MCP roundtrip | `codegraph` feature 下 catalog 对照 smoke（可选）；`--lib` 偶发 cursor_hooks 并行 temp 环境 flake |
| **P10 smoke（§6.4）** | ✅ done · **gap 0** | Batch H 跨宿主 / workflow / isolation / shutdown smoke 全批；`subagent_real_process_spawn_terminate_smoke`（`ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE=1`）；`workflow_state_isolation_smoke`；`subagent_resource_leak_detection` | — |
| **P7 CLI 薄壳 / B3 下沉** | ✅ done | B7 薄壳预算；`live_execute` / `stdio_op_registry` / `stdio_dispatch` / `json_payload` / `trace_transport` / **`trace_attach` / `trace_stream_io`** 已迁 B3；`runtime_ops.inc` **117L** | — |
| **P8 de-tmux** | ✅ done | `grep tmux core/` → **0** | — |
| **P4 host-projection** | ✅ done | `/team` fail-closed；`host_provider_routing_aliases` | — |
| **P4 handlers 拆分** | ✅ done · **gap 0** | `handlers/review_gate.rs`（1345L）、`outbound.rs`（150L）、`stop.rs`（314L）；`handlers.rs` **1172L** | 旧 `handlers_parts/*.inc.rs` 可择机清理 |
| **CG W1–W4** | ✅ done | crate + 六工具 MCP + sync/watcher + rayon/prepared stmt/schema v2 | — |
| **CG-5 skill 集成** | ✅ done | planx / implementx / verifyx / code-review-deep `allowed_tools` + 场景化 CodeGraph 节；`smoke_codegraph_semantic_dispatch_tests.rs` | — |
| **CG E2E 五宿主 stdio** | ✅ done | `smoke_codegraph_five_host_stdio_e2e_tests.rs`；opt-in `ROUTER_RS_CODEGRAPH_STDIO_E2E=1`；闭集 5 宿主 tools/list + `codegraph_status` | CI 默认跳过（opt-in 约 157s） |
| **EV-1 – EV-6** | ✅ done | TOML 配置、`evolution_observer`、RFV journal、`goal_prediction` closeout dry-run + **`PredictionOutcome` journal**（match/mismatch 均写） | closeout 仍 warn-only（不 block） |
| **EV-7** | ✅ done | Codex/Claude hook journal + PostToolUse duration（五宿主含 Cursor）+ RFV 分桶 analyze + supervisor idle→evolution analyze dry-run | `phase-ev7-closeout.json` 旧 gap 已被 tail/full 覆盖 |
| **P9 文档轨** | ✅ slice | ops 模块化 B0–B11；契约 / policy 链接 **24 passed**；manifest 快审（slides）；tmux schema 0；过期文件清扫 **82** 删 | 宽口径 30+ 路径；depends_on 全图校验 |
| **P3 HostProvider** | ✅ done | trait + 五宿主 impl + routing aliases + pre_tool_use_guard 集成测 | `phase-p3-hostprovider-slice.json` 仍标 in_progress — **lane-note 过期** |
| **P2 退役面** | ✅ done | 五宿主闭集验收；antigravity manifest；`ref_corpus_tool_rs` 构建 | 历史投影 stub / 文档注释（有意保留） |
| **core-policy 物理迁移** | ✅ done | hook/review 模块迁 `core-policy`；`review_gate` CLI 留 router-rs 胶合 | — |
| **workspace 九 crate** | ⏸ | — | 15–20d 量级，未启动 |
| **Git** | — | — | 未 commit |

---

## 测试基线（2026-06-09 审计跑数）

| crate / 套件 | 结果 | 备注 |
|--------------|------|------|
| `router-rs --lib` | **1050 passed**, 14 ignored | 全绿；`CARGO_TARGET_DIR=/tmp/skill-cargo-final` |
| `router-rs smoke_codegraph` | **7 passed** | feature `codegraph`；stdio E2E opt-in 另验 |
| `codegraph-rs` | **25 passed** | 含 W3/W4/e2e minimal |
| `routing-engine` | **20 passed** | |
| `evolution-rs` | **9 passed** | 含 `prediction_outcome` 解析 |
| `core-policy` + `framework-kernel` | **合并绿** | |
| `documentation_contracts` + `policy_markdown_links` + `policy_cursor_rules_links` | **24 passed** | |

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
| `p4-cursor-hooks-review-gate-split` | review_gate→inc | ✅ `handlers_review_gate.inc.rs` 32KB；`handlers.rs` 64KB |
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

HostProvider 骨架 · Telemetry MPSC · PreToolUse Fallback · routing 热更新 · evolution_observer + idle 触发 · codegraph 六工具 + sync/watcher + **五宿主 stdio E2E** · session_supervisor · **tmux 0 残留** · P7 `runtime_ops.inc` 117L · P10 gap 0 · P0 smoke 收口 · CG-5 skill 集成 · **P4 handlers gap 0** · **EV-6 PredictionOutcome journal**

## 宿主真源

闭集成员与退役 id 仅以 [`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) → `host_targets.supported` 为准。

**验收**：`runtime_registry_closed_set_is_canonical_five_hosts` 等 — 3/3 passed（2026-06-08 基线，registry 未变）。

---

## 历史轮次（归档 · 勿作当前状态）

<details>
<summary>2026-06-08 – 2026-06-09 切片日志（已 supersede 者见上表）</summary>

### 2026-06-09 · P4 handlers 首片 + review_gate inc

- `handlers/stop_closeout.rs` + `handlers_review_gate.inc.rs` 抽出；`cargo test -p router-rs --lib` **1071 passed**
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
