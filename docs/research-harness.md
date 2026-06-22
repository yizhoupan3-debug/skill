---
parent: docs/README.md
depends_on:
  - research/routing-contracts.md
version: unified-v9（精简版）
---

## 19. 科研 Harness 系统 (Research Harness)

> 本规约是科研 Harness 系统的**唯一权威真源**，覆盖科研 Skill 路由契约、学术检索接入、研究工作区 CLI、
> 分层日志体系、Smoke Test 新鲜度守卫、Claim Drift 防护，以及**与 Loop 工程统一后的研究感知循环**。
> 所有代码变更须以本文档为第一断言断点。

---

### 19.1 系统总览与拓扑

拓扑包含两条独立的执行路径——**交互式**（人工在回路中）和**自动循环**（loop-auto 无人值守）——以及两者之间的 **Barrier Escalation 桥接**。

```
                              用户输入
                                  │
                  ┌───────────────┴───────────────────┐
                  ▼                                    ▼
        交互式路径（人工驱动）                自动循环路径（loop-auto）
                  │                                    │
          NL 路由 / $ref                          LOOP_REGISTRY.json
                  │                                    │
        ┌─────────┼──────────┐              ┌──────────┼──────────┐
        ▼         ▼          ▼              ▼          ▼          ▼
 research-  research-   autoresearch    loop-code  loop-triage   ║
 discovery  execution   (skill wrap)    fix        ...          ║
   │           │            │                                   ║
   │           │            ├── log layer (§19.5)               ║
   │           │            ├── smoke test (§19.6)              ║
   │           │            ├── drift detect (§19.7)            ║
   │           │            │                                   ║
   └───────────┴────────────┼──── Barrier Escalation ───────────╣
                            │   当循环遇到硬指标突破不了时       ║
                            │   → autoresearch init <problem>   ║
                            │   → research-discovery             ║
                            │   → 返回候选方案 → 恢复循环       ║
                            ▼                                   ║
                     research-aware loop 模式 (§19.9)           ║
                                                                ║
             统一状态层 ─── LOOP_RUN_STATE ↔ research-state.yaml ╝
```

**关键设计原则**：
1. 交互式和自动循环共享同一套科研组件（research-discovery / research-execution / autoresearch），区别在于驱动方式
2. **Barrier Escalation** 是循环路径到研究路径的桥接——两者不合并为同一进程，而是通过结构化 handoff 通信
3. 研究路径的输出（hypothesis、claim、evidence）可被循环路径消费，反之亦然

**依赖关系**：

```
research-discovery / research-execution
    → paperplain MCP (学术元数据查询)
    → browser-mcp (web_search 回退)
    → skill routing system (NL 路由 ± $ref)

autoresearch (skill wrapper)
    → core/research-harness/src/bin/autoresearch.rs (Rust CLI，跨 skill 共享)
    → core/research-harness/src/log/ (分层日志，§19.5)
    → research-state.yaml (研究状态真源)
    → research-discovery / research-execution (§19.4.4 handoff)

deep-research (独立 skill，Web 优先)
    → browser-mcp (web_search + web_fetch)
    → .claude/workflows/deep-research.js (native workflow)

loop-auto + research 模式 (§19.9)
    → LOOP_REGISTRY.json (注册)
    → LOOP_RUN_STATE ↔ research-state.yaml (状态桥接)
    → autoresearch (研究执行)
    → research-discovery (文献查找)
```

---


### 路由契约

research-discovery 和 research-execution 路由契约详见 [research/routing-contracts.md](research/routing-contracts.md)。

### 19.4 autoresearch — 研究工作区 Skill（含 Loop 统一）

#### 19.4.1 Skill 元数据

```yaml
name: autoresearch
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: optional
user-invocable: true       # 可通过 /autoresearch 调用
disable-model-invocation: false
trigger_hints:
  - 研究工作区
  - 科研工作区
  - claim 管理
  - 新颖性声明
  - 假设管理
  - 实验记录
  - research workspace
  - novelty gate
  - 实验反思
  - 科研回顾
  # loop 统一相关
  - 自动研究
  - loop 研究
  - barrier research
  - 瓶颈研究
  - 突破方向
alias_tokens:
  - workspace
  - claim
  - novelty
  - hypothesis
  - 实验
  - 假设
  - 新颖性
  - loop
  - barrier
  - 瓶颈
  - 突破
```

#### 19.4.2 Lanes

| Lane | CLI 子命令 | 描述 |
|------|-----------|------|
| `workspace_init` | `init` | 初始化新科研方向（project + question + mode） |
| `workspace_resume` | `status / next / resume` | 恢复/查看当前工作区 |
| `claim_drafting` | `draft-claims / compare-claim / set-novelty-gate` | 新颖性声明全生命周期 |
| `external_research` | `research-claim / research-all` | 外部学术检索（回退到 research-discovery） |
| `hypothesis_tracking` | `add-hypothesis / list-hypotheses` | 假设增删改查 |
| `run_recording` | `record-run` | 实验记录（含环境指纹 + git 溯源） |
| `reflection` | `reflect` | 实验反思 + claim drift 检测 |
| `log` | `log:record / log:search / log:insight / log:connect` | 分层日志记录与检索 |
| `smoke_test` | `smoke-test` | 新鲜度守卫 |
| `barrier_escalation` | `barrier <problem>` | **新**：当 auto-loop 遇到硬指标突破不了时调用 |
| `sync` | `sync` | 同步到 artifact |

#### 19.4.3 barrier_escalation lane 详细契约

这是 autoresearch 与 loop 工程统一的核心桥梁，详见 `skills/autoresearch/SKILL.md` 中的 barrier escalation section。
输出格式为 BARRIER_REPORT.json（schema_version: barrier-report-v1）。

#### 19.4.4 后端实现

所有 lane 通过 `cargo run -p research-harness --bin autoresearch -- <subcommand>` 调用 `core/research-harness/src/bin/autoresearch.rs` CLI。

工作区数据：
- 状态：`<workspace>/research-state.yaml`（schema_version: 4）
- 实验运行：`<workspace>/run-ledger.jsonl`
- 日志：`artifacts/research-log/` → §19.5
- Barrier 报告：`artifacts/research-barrier/` → §19.9.3

#### 19.4.5 跨 Skill 手递

```
autoresearch → $research-discovery    深度文献调研（barrier escalation 已内置自动学术检索；人工深入调研通过本路由）
autoresearch → $research-execution    实验设计/验证
autoresearch → $paper-workbench       手稿级产出
autoresearch → loop runner            barrier_report 格式输出 → 恢复循环
```

---

### 19.5 分层科研日志体系

#### 19.5.1 文字层 — `artifacts/research-log/`

```
artifacts/research-log/
├── INDEX.md                       # 所有方向的索引 + 时间线
├── YYYY-MM/
│   ├── YYYY-MM-DD_direction-name.md
│   └── ...
└── tags/
    └── tag-name.md                # 按标签聚合
```

**每篇日志格式**：

```markdown
# YYYY-MM-DD: 方向名

## 初始问题
## 探索路径（分支/决策点）
## 关键发现
## 未解决的问题
## 关联 claim / hypothesis
## 下次切入建议
```

**INDEX.md 格式**：

```markdown
# Research Log Index

| 日期 | 方向 | 状态 | 标签 | 关联 barrier |
|------|------|------|------|-------------|
| YYYY-MM-DD | direction-name | active/abandoned/concluded | tag1, tag2 | barrier-id |
```

#### 19.5.2 压缩数据库层 — SQLite FTS5

完整 schema 定义见 `core/research-harness/src/log/db.rs` 中的 `SCHEMA_VERSION` 常量和建表语句。

**操作命令**（通过 `cargo run -p research-harness --bin research-log -- <subcommand>` 调用）：
- `record` — 记录探索
- `search <query>` — FTS5 检索
- `add-finding --entry-id <id>` — 记录发现
- `connect <a> <b>` — 连接两个方向
- `barrier [--loop-id]` — 查询 barrier 报告
- `export --format <json|csv|obsidian>` — 导出日志
- `neighbors/path/viz/graph-stats/route` — 知识图谱遍历（见 §19.13）

> **注意**：`log:route <barrier-id>`（从 barrier 追溯完整研究路径）已通过 `research-log route --barrier-id <id>` 实现，见 §19.13。

#### 19.5.3 两者同步规则

- 文字层为**不可丢弃的人读真源**
- SQLite 层为**可重建的机器检索索引**
- 每写入一篇日志 / barrier 报告，须同时更新两者
- 文字层首行 frontmatter 记录 SQLite ID，SQLite 行记录文件路径

---

### 19.6 Smoke Test 新鲜度守卫

#### 19.6.1 注册测试集

文件：`artifacts/research-log/smoke-tests.json`

```json
{
  "schema_version": "research-smoke-v1",
  "queries": [
    {
      "id": "unique-query-id",
      "source": "arxiv|openalex|crossref|pubmed|doaj",
      "query": "URL-encoded API query string",
      "expected_min_results": 3,
      "expected_freshness_days": 180,
      "expected_has_pdf": true,
      "related_directions": ["direction-name"],
      "related_barriers": ["barrier-id"]     // 可选：关联到 barrier
    }
  ],
  "barrier_extends": {
    "barrier-id": {                          // 可选：barrier 专用的自动化测试集
      "queries": [...]
    }
  }
}
```

#### 19.6.2 执行命令

```
autoresearch smoke-test                    # 跑全部
autoresearch smoke-test --source arxiv     # 按源过滤
autoresearch smoke-test --barrier id       # 按 barrier 过滤
```

输出：`artifacts/research-log/smoke-test-results.jsonl`

```jsonl
{"id":"attn-2026","passed":false,"results_count":2,"expected_min":3,"stale":true,"freshness_days":365,"error":"expected ≥3, got 2","timestamp":"..."}
```

#### 19.6.3 回归检测

每次运行对比上次结果，以下情况标记为 REGRESSION：
- 之前 passed 的查询变为 failed
- 结果数量下降超过 50%
- 新鲜度窗口扩大超过 2×

#### 19.6.4 barrier 触发的自动 smoke test

当 loop runner 触发 barrier escalation 时，**自动**针对该 barrier 的 `related_barriers` 查询跑 smoke test，
确保研究开始时使用的文献数据是新鲜的。

---

### 19.7 Claim Drift 防护

#### 19.7.1 数据结构

在 `research-state.yaml` 中强化锚定字段：

```yaml
current_direction:
  original_question: "..."             # 不可变
  last_reaffirmed: "ISO8601"           # 每次实验前更新
  deviation_warning_count: 0           # 超过阈值强制提醒

active_hypothesis:
  id: "hyp-001"
  original_claim: "..."                # 不可变
  falsifiable_prediction: "..."        # 不可变
  drift_guard:
    last_checked: "ISO8601"
    deviation_log: []                  # [{timestamp, deviation, corrected: bool}]
    perimeter: "仅限于条件 X，不推广到 Y"
```

#### 19.7.2 drift 检测算法

对 `current_direction.original_question` 与 `active_hypothesis` 做三类偏移检测：
- **structure drift**：原始问题与当前假设的语义相似度 < 0.5
- **perimeter breach**：最近运行记录超出 falsifiable_prediction 的 perimeter
- **question drift**：运行描述的问题与 original_question 偏离

聚合三项加权得分得到 `drift_score`，超过阈值则输出警告并追加 `deviation_log`。

#### 19.7.3 阈值与响应

`drift_score` 分为四个级别：< 0.3 静默记录，0.3–0.6 输出提示，0.6–0.8 警告并递增 `deviation_warning_count`，≥ 0.8 或累计警告 ≥ 3 阻断执行并等待确认。

#### 19.7.4 循环中的 drift 检测

当 autoresearch 被 loop runner 通过 barrier escalation 调用时，自动执行 drift 检测（以 barrier 描述为 original_question），若 drift_score > 0.6 则追加到 barrier report 的 attempted 列表。

---

### 19.8 loadout 激活策略

`skills/SKILL_LOADOUTS.json` 中 `research_loadout` 的激活策略：

```json
"research_loadout": {
    "tier_activation": "default",          // 开机即加载
    "skill_slugs": [
        "research-discovery",
        "research-execution",
        "autoresearch",                    // 新增
        "paper-workbench",
        "citation-management",
        "statistical-analysis",
        "experiment-reproducibility",
        "math-derivation",
        "scientific-figure-plotting",
        "deep-research",
        "pdf",
        "plan-mode"
    ],
    "entry_map": "skills/research-discovery/SKILL.md"
}
```

`default_surface_loadout` 与 `research_loadout` 为**可叠加**关系（非互斥）。

---

### 19.9 Research-Aware Loop 模式

> 本节定义 autoresearch 与 loop 工程统一的**核心契约**。
> 背景：当 loop-auto 循环遇到硬指标突破不了时，需要自动升级为系统化研究，
> 找到候选方案后恢复循环。

#### 19.9.1 Loop 模式 Catalog

loop-engine crate 的 loop mode catalog（见 `core/loop-engine/src/`）中应增加：

| Skill | Cadence | Safety | 产出 | 触发条件 |
|-------|---------|--------|------|---------|
| loop-research-barrier | 按需（on escalation） | L2 | barrier report + candidate list | consecutive_failures ≥ N |
| loop-hypothesis-test | 按配置 | L2 | hypothesis verification result | 新 hypothesis 就绪 |
| loop-literature-watch | 1w | L1 | new papers digest | cron |
| loop-claim-refresh | 1w-2w | L1 | drift detection report | cron |

#### 19.9.2 LOOP_REGISTRY.json 扩展

在 `configs/framework/LOOP_REGISTRY.json` 中增加 `research` 节字段：`barrier_threshold`（连续失败 N 次后触发）、`escalation_target`、`max_research_time_min`、`auto_resume`、`require_human_approval`。

#### 19.9.3 状态真源与路径

```
artifacts/
├── loop/<loop-id>/                     ← loop 执行状态
│   ├── LOOP_RUN_STATE.json
│   ├── evidence/<action-id>/
│   └── reports/<run-id>.md
├── research-barrier/                   ← barrier 爆发的研究工件
│   └── <barrier-id>/
│       ├── BARRIER_REPORT.json         ← §19.4.3 格式
│       ├── research-state.yaml         ← autoresearch 状态
│       └── smoke-test-results.jsonl    ← barrier 触发时的快照
├── research-log/                        ← §19.5 分层日志
└── current/<task_id>/                   ← 现有 closeout 路径
    └── closeout/<task_id>.json
```

#### 19.9.4 执行流程

Loop Runner 检测到 consecutive_failures ≥ barrier_threshold 后，若 LOOP_REGISTRY.research_enabled 为 true，则构造 barrier description 并调用 `autoresearch barrier <description>`。autoresearch 执行 barrier escalation 流程（§19.4.3）并产出 BARRIER_REPORT.json。runner 读取报告后根据 auto_resume 配置自动选择推荐候选恢复循环，或进入 ESCALATED（等人）状态。若 research_enabled 为 false，则按现有 escalation 路径处理。

#### 19.9.5 Loop 安全越界保护

research escalation 允许修改 loop scope 外的文件，但 barrier 报告中需标注 out_of_scope_changes。恢复循环时 loop runner 对 out-of-scope 变更做 `git stash`（不销毁，仅隔离）。

---

### 19.10 测试契约

| 测试点 | 覆盖内容 | 最低通过数 |
|--------|---------|-----------|
| 路由评估 | `evaluate_routing_cases` 加载 research 领域测试用例 | 每方向 ≥3 case |
| research-discovery NL 路由 | 隐含 barrier 信号的 30+ 条 trigger_hints 全部命中 | 30/30 |
| research-execution NL 路由 | 20+ 条 trigger_hints 全部命中 | 20/20 |
| autoresearch CLI 集成 | 完整循环、外部研究、批量研究 + gate 推荐 | 3 cases |
| autoresearch barrier 子命令 | barrier init → research → report 全流程 | 1 case |
| smoke test | 所有注册查询返回结果 + 新鲜 | 每查询 1 case |
| drift 检测 | 结构偏移/边界违例/问题漂移 三类场景 | 3 cases |
| loadout 合并 | `default_surface_loadout` + `research_loadout` 同时加载 | 1 case |
| loop research escalation (new) | loop_runner → autoresearch barrier → 恢复循环 | 1 case |

---

### 19.12 文件清单与维护责任

| 文件 | 维护者 | 更新触发 |
|------|--------|---------|
| `docs/research-harness.md`（本文件） | 框架维护者 | 所有科研 Harness 变更 |
| `skills/research-discovery/SKILL.md` | Skill 作者 | 路由契约/lane/barrier 入口变更 |
| `skills/research-discovery/references/academic-sources.md` | Skill 作者 | 学术源新增/更新 |
| `skills/research-execution/SKILL.md` | Skill 作者 | 路由契约/lane 变更 |
| `skills/autoresearch/SKILL.md` | Skill 作者 | workspace lane / barrier escalation 变更 |
| `core/research-harness/src/bin/` | Rust 开发者 | CLI 功能变更（含 barrier 子命令） |
| `core/research-harness/src/log/`（已实现） | Rust 开发者 | 日志系统 + FTS5 查询 |
| `artifacts/research-log/smoke-tests.json` | 研究员 | 新查询/方向/barrier 注册 |
| `artifacts/research-barrier/` | loop runner + autoresearch | barrier escalation 自动写入 |
| `configs/framework/LOOP_REGISTRY.json` | 框架维护者 | loop research 模式注册 |
| `configs/framework/ROUTING_SIGNAL_MARKERS.json` | 框架维护者 | barrier 信号定义 |
| `configs/framework/NL_ROUTE_ADJUSTMENTS.json` | 框架维护者 | barrier boost 规则 |
| `SKILL_LOADOUTS.json` | 框架维护者 | loadout 变更 |
| `SKILL_ROUTING_RUNTIME.json` | 框架维护者 | 路由注册变更 |
| `docs/spec/loop-architecture.md` | 框架维护者 | loop patterns catalog 中 research 模式 |

### 19.13 Research Knowledge Graph

> 在扁平日志之上建立可查询的图结构，包括条目关系遍历、知识实体提取、跨工作区索引、图可视化和 barrier 路径追溯。

#### 19.13.1 系统设计

系统由五层组成：connections 存储（条目间关系）、graph 遍历引擎（BFS/最短路径等）、实体提取（5 组 regex）、跨工作区 Hub（`~/.claude/research-knowledge-hub.db`）、可视化（ASCII / Graphviz DOT）。完整架构见 `core/research-harness/src/log/`。

#### 19.13.2 核心数据结构

connections 表（v2→v3 migration）增加 `weight` 和 `confidence` 字段。entities、entity_relations、entry_entities 三表的完整 schema 见 `core/research-harness/src/log/db.rs` 中的建表语句。

#### 19.13.3 CLI 命令

所有命令通过 `research-log <subcommand>` 或 `autoresearch log:<subcommand>` 调用：

| 命令 | 功能 |
|------|------|
| `neighbors <entry-id> [--relation]` | 显示 entry 的直接连接 |
| `path --from <id> --to <id> [--max-depth]` | BFS 最短路径 |
| `subgraph <entry-id> [--max-depth] [--format text\|dot]` | 子图提取 |
| `viz [--entry-id] [--max-depth] [--format]` | ASCII / DOT 可视化 |
| `graph-stats` | 全图统计信息 |
| `route --barrier-id <id> [--max-depth]` | Barrier 路径追溯 |
| `extract-entities <entry-id>` | 自动提取知识实体 |
| `add-entity <name> [--kind] [--description]` | 手动添加实体 |
| `search-entities <query> [--limit]` | FTS5 实体搜索 |
| `link-entities <a> <b> --relation <rel>` | 链接实体 |
| `hub-register [--path] [--name]` | 注册到 Hub |
| `hub-index [--path]` | 索引到 Hub |
| `hub-search <query>` | 跨工作区搜索 |
| `hub-list` | 列出工作区 |

#### 19.13.4 跨工作区 Hub 与实体提取

Hub 数据库位于 `~/.claude/research-knowledge-hub.db`，用作全局研究记忆。schema 包含 `workspace_index`、`hub_entries`、`hub_entries_fts` 三张核心表。

实体提取无外部 NLP 依赖。5 组 hardcoded regex 覆盖量化金融/ML 常见 vocabulary：
- **method** (30+): CNN, LSTM, Transformer, EWMA, PCA, GARCH, Attention, LoRA...
- **dataset** (15+): SQuAD, ImageNet, MNIST, CIFAR, GLUE, CRSP, Compustat...
- **metric** (35+): accuracy, F1, AUC, Sharpe, IC, Rank IC, MSE, KL divergence...
- **model** (20+): GPT-4, BERT, ResNet, Llama, Fama-French, Factor Model...
- **tool** (15+): PyTorch, TensorFlow, scikit-learn, Hugging Face...

提取流程：拼接 entry.question + findings.content + tags → 5 组 regex 匹配 → dedup → upsert to entities 表。

---

### 19.14 与框架其他规约的关系

| 关联文档 | 关系 |
|---------|------|
| `docs/adr/010-ideal-architecture-v10.md` | 统一规约：7 层架构、路由与插件契约、运行时、安全生命周期、可观测性 |
| `core-policy` (代码) | Review gate 状态机、安全策略 |
| `runtime-core` (代码) | 运行时编排、closeout gate |

---

### 19.15 统一 Rust Crate 架构（`core/research-harness/`）

> 模块结构与依赖关系已迁移至 `core/research-harness/README.md`。
