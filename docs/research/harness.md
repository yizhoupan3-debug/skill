---
version: unified-v9（精简版）
---

# 科研 Harness 系统 (Research Harness)

> 本规约是科研 Harness 系统的**唯一权威真源**，覆盖科研 Skill 路由契约、学术检索接入、研究工作区 CLI、分层日志体系、Smoke Test 新鲜度守卫、Claim Drift 防护，以及**与 Loop 工程统一后的研究感知循环**。所有代码变更须以本文档为第一断言断点。

---

## 1. 系统总览与拓扑

两条独立执行路径——**交互式**（人工在回路中）和**自动循环**（loop-auto 无人值守）——通过 **Barrier Escalation 桥接**通信。交互式路径走 NL 路由或 `$ref`；自动循环路径走 `LOOP_REGISTRY.json`。两者共享同一套科研组件（research-discovery / research-execution / autoresearch），区别仅在于驱动方式。

```
交互式路径：NL 路由 → research-discovery / research-execution / autoresearch
自动循环：LOOP_REGISTRY.json → loop-code-fix / loop-triage → 研究升级
  ↕ Barrier Escalation — 循环遇到硬指标 → autoresearch barrier → research-discovery → 候选方案 → 恢复循环
统一状态层：LOOP_RUN_STATE ↔ research-state.yaml
```

**依赖拓扑**：

| 组件 | 依赖 |
|------|------|
| research-discovery / research-execution | → paperplain MCP, browser-mcp, skill routing |
| autoresearch | → `core/research-harness/src/bin/autoresearch.rs`, research-state.yaml, research-discovery |
| deep-research | → browser-mcp, `skill_route` → 并行 agent 编排 |
| loop-auto + research | → LOOP_REGISTRY.json, LOOP_RUN_STATE ↔ research-state.yaml, autoresearch |

---

## 2. 路由契约

### 2.1 research-discovery 路由契约

**元数据**：`routing_layer: L2`, `routing_owner: owner`, `routing_gate: none`, `routing_priority: P2`, `session_start: preferred`。`user-invocable: false`（保持隐藏），`disable-model-invocation: false`。

**trigger_hints**（三类场景全覆盖）：

| 场景 | 示例 hints |
|------|-----------|
| 科研方向调研 | `调研方向` `文献综述` `研究方向` `主题深挖` `研究综述` `相关定理` `理论背景` |
| 论文/学术搜索 | `查论文` `找文献` `有哪些论文` `学术搜索` `最新进展` `sota` `literature` |
| 性质探索 | `未知性质` `性质不清楚` `类比` `该用什么理论` `深度调研` |
| barrier 升级 | `突破不了` `硬指标` `瓶颈` `卡住了` `stuck` |

**alias_tokens**：`["论文", "文献", "学术", "调研", "方向", "landscape", "sota", "survey", "gap", "突破", "瓶颈", "stuck"]`

**路由评分**：`trigger_hint_per_match: 20.0`, `layer_threshold_L2_L3: 14.0`（1 个 hint 即可过阈值）。barrier 信号通过 `NL_ROUTE_ADJUSTMENTS.json` 额外 boost research-discovery。

**external_research lane**：first-class `paperplain` MCP（`search_research` / `fetch_paper` / `find_paper_by_title`）；fallback 手动 HTTP 调用五源。查询结果必须携带 `freshness` 元数据（fetch_time, oldest/newest_result_date, stale flag）。stale 判定：newest < now-180d 或 coverage > 730d 且含年度关键词则 `stale: true`，不得作为强证据。

**Lane 手递**：`research_question` → `external_research` 或 `$research-execution` → `math_background_inquiry` → `$math-derivation` → `paper_handoff` → `$paper-workbench`。

**Barrier Escalation 入口**：检测到 barrier 信号 + 实施上下文 → research-discovery 获 +20 boost → 初始化研究方向界定 → 产出证据地图 + 候选方案 → 手递回 execution。

### 2.2 research-execution 路由契约

**元数据**：同 research-discovery（`L2`, `P2`, `user-invocable: false`）。**Lanes**：`experiment_design`, `math_verification`, `math_modeling`, `code_verification`, `reproducibility`。

**研究层次晋级规则**：discovery（发现问题 → 回 research-discovery）→ execution（需新文献或 barrier → 回 research-discovery）→ paper → `$paper-workbench`。

---

## 3. autoresearch — 研究工作区 Skill（含 Loop 统一）

**元数据**：`L2`, `P2`, `user-invocable: true`（可通过 `/autoresearch` 调用）。trigger_hints 覆盖 `研究工作区`, `claim 管理`, `新颖性声明`, `自动研究`, `barrier research`, `瓶颈研究` 等。alias_tokens：`[workspace, claim, novelty, hypothesis, loop, barrier]`。

**Lanes**：

| Lane | CLI 子命令 | 描述 |
|------|-----------|------|
| workspace_init/resume | `init / status / next / resume` | 初始化/恢复科研方向 |
| claim_drafting | `draft-claims / compare-claim / set-novelty-gate` | 新颖性声明全生命周期 |
| external_research | `research-claim / research-all` | 外部学术检索 |
| hypothesis_tracking | `add-hypothesis / list-hypotheses` | 假设管理 |
| run_recording | `record-run` | 实验记录（环境指纹 + git 溯源） |
| reflection | `reflect` | 实验反思 + claim drift 检测 |
| log | `log:record / log:search / log:insight / log:connect` | 分层日志 |
| smoke_test | `smoke-test` | 新鲜度守卫 |
| barrier_escalation | `barrier <problem>` | auto-loop 遇到硬指标时调用 |
| sync | `sync` | 同步到 artifact |

所有 lane 通过 `cargo run -p research-harness --bin autoresearch -- <subcommand>` 调用。工作区数据：`<workspace>/research-state.yaml`（schema_version: 4）、`<workspace>/run-ledger.jsonl`。输出日志：`artifacts/research-log/`；Barrier 报告：`artifacts/research-barrier/`。

**跨 Skill 手递**：autoresearch → `$research-discovery`（深度文献）, → `$research-execution`（实验设计）, → `$paper-workbench`（手稿产出）, → loop runner（barrier_report 格式输出）。

---

## 4. 分层科研日志体系

文字层 `artifacts/research-log/`（INDEX.md + `YYYY-MM/YYYY-MM-DD_direction-name.md` + `tags/`）+ SQLite FTS5 压缩层（`core/research-harness/src/log/db.rs`）。每篇日志含：初始问题、探索路径、关键发现、未解决问题、关联 claim/hypothesis、下次切入建议。文字层为不可丢弃的人读真源，SQLite 为可重建的检索索引，两者双向同步（frontmatter SQLite ID × SQLite 行文件路径）。

操作命令：`research-log record / search / add-finding / connect / barrier / export`。图操作命令见 [§11](#11-research-knowledge-graph)（`neighbors/path/route` 等）。

---

## 5. Smoke Test 新鲜度守卫

注册于 `artifacts/research-log/smoke-tests.json`（schema_version: `research-smoke-v1`），每条查询含 source、query、expected_min_results/freshness_days/has_pdf。执行：`autoresearch smoke-test [--source <src>] [--barrier <id>]` → 输出 `smoke-test-results.jsonl`。回归检测：之前 passed 变 failed、结果下降 >50%、新鲜度窗口扩大 >2× 标记为 REGRESSION。loop barrier 触发时**自动**跑关联查询的 smoke test。

---

## 6. Claim Drift 防护

在 `research-state.yaml` 中强化锚定字段：`original_question`（不可变）、`last_reaffirmed`、`active_hypothesis`（含 `original_claim`、`falsifiable_prediction`、`drift_guard.perimeter`）。三类偏移检测：**structure drift**（语义相似度 < 0.5）、**perimeter breach**（超出 falsifiable_prediction）、**question drift**（运行描述偏离原始问题）。`drift_score` 四级别：<0.3 静默 / 0.3-0.6 提示 / 0.6-0.8 警告 / ≥0.8 或累计 ≥3 阻断。loop 中 autoresearch 被 barrier escalation 调用时自动执行 drift 检测。

---

## 7. loadout 激活策略

`skills/SKILL_LOADOUTS.json` 中 `research_loadout` 为 `tier_activation: "default"`，包含 12 个 skill slug（research-discovery, research-execution, autoresearch, paper-workbench 等）。`default_surface_loadout` 与 `research_loadout` 为可叠加关系。

---

## 8. Research-Aware Loop 模式

| Skill | Cadence | Safety | 产出 | 触发条件 |
|-------|---------|--------|------|---------|
| loop-research-barrier | 按需（on escalation） | L2 | barrier report + candidate list | consecutive_failures ≥ N |
| loop-hypothesis-test | 按配置 | L2 | hypothesis verification result | 新 hypothesis 就绪 |
| loop-literature-watch | 1w | L1 | new papers digest | cron |
| loop-claim-refresh | 1w-2w | L1 | drift detection report | cron |

`LOOP_REGISTRY.json` 扩展 `research` 节：`barrier_threshold`、`escalation_target`、`max_research_time_min`、`auto_resume`、`require_human_approval`。执行流程：Loop Runner 检测到 consecutive_failures ≥ barrier_threshold（`research_enabled: true`）→ `autoresearch barrier <description>` → BARRIER_REPORT.json → 自动恢复或进入 ESCALATED。research escalation 允许修改 loop scope 外文件，但 barrier 报告中须标注 `out_of_scope_changes`；恢复循环时 git stash 隔离。

**目录结构**：`artifacts/loop/<loop-id>/`（loop 执行状态）、`artifacts/research-barrier/<barrier-id>/`（BARRIER_REPORT.json + research-state.yaml + smoke-test-results.jsonl）、`artifacts/research-log/`（分层日志）。

---

## 9. 测试契约

| 测试点 | 覆盖内容 | 最低通过数 |
|--------|---------|-----------|
| 路由评估 | `evaluate_routing_cases` 加载 research 领域用例 | 每方向 ≥3 |
| research-discovery NL | 隐含 barrier 信号的 30+ 条 trigger_hints | 30/30 |
| research-execution NL | 20+ 条 trigger_hints | 20/20 |
| autoresearch CLI 集成 | 完整循环、外部研究、批量研究 + gate 推荐 | 3 |
| autoresearch barrier | barrier init → research → report 全流程 | 1 |
| smoke test | 所有注册查询返回结果 + 新鲜 | 每查询 1 |
| drift 检测 | 三类偏移场景 | 3 |
| loop research escalation | loop_runner → autoresearch barrier → 恢复循环 | 1 |

---

## 10. 文件清单与维护责任

| 文件 | 维护者 | 更新触发 |
|------|--------|---------|
| `docs/research/harness.md` | 框架维护者 | 所有科研 Harness 变更 |
| `skills/research-discovery/SKILL.md` | Skill 作者 | 路由契约/lane/barrier 入口变更 |
| `skills/research-execution/SKILL.md` | Skill 作者 | 路由契约/lane 变更 |
| `skills/autoresearch/SKILL.md` | Skill 作者 | workspace lane / barrier 变更 |
| `core/research-harness/src/bin/` | Rust 开发者 | CLI 功能变更 |
| `artifacts/research-log/smoke-tests.json` | 研究员 | 新查询/方向/barrier |
| `configs/framework/LOOP_REGISTRY.json` | 框架维护者 | loop research 模式注册 |
| `configs/framework/NL_ROUTE_ADJUSTMENTS.json` | 框架维护者 | barrier boost 规则 |

---

## 11. Research Knowledge Graph

| 组件 | 位置 | 核心能力 |
|------|------|---------|
| Connections 存储 | `core/research-harness/src/log/db.rs` | 条目间关系（weight + confidence） |
| 图遍历引擎 | 同上 | BFS 最短路径、子图提取 |
| 实体提取 | 同上 | 5 组 hardcoded regex（method/dataset/metric/model/tool） |
| 跨工作区 Hub | `~/.claude/research-knowledge-hub.db` | 全局研究记忆、跨工作区搜索 |
| 可视化 | CLI | ASCII / Graphviz DOT |

CLI 命令：`research-log neighbors / path / subgraph / viz / graph-stats / route --barrier-id / extract-entities / add-entity / search-entities / link-entities / hub-register / hub-index / hub-search / hub-list`。

---

## 12. 与框架其他规约的关系

| 关联文档 | 关系 |
|---------|------|
| `docs/architecture.md` | 八层架构、路由契约、运行时 |
| `core-policy` (代码) | Review gate 状态机、安全策略 |
| `runtime-core` (代码) | 运行时编排、closeout gate |

---

## 13. Crate 架构

模块结构见 `core/research-harness/README.md`。
