---
parent: docs/README.md
depends_on:
  - research/routing-contracts.md
version: unified-v9
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

这是 autoresearch 与 loop 工程统一的核心桥梁。

**触发条件**（loop → autoresearch）：

```
on_verify_fail after retries exhausted
    → closeout 记录含 "hard_barrier" 标签
    → loop runner 检测到 consecutive_failures ≥ threshold
    → 自动调用: autoresearch barrier <problem-description>
```

**执行流程**：

```
1. autoresearch barrier init
   → 创建临时研究 workspace
   → 填充 barrier description + 失败实验上下文
   → 写入 research-state.yaml

2. literature review (自动化学术检索)
   → Semantic Scholar + arXiv HTTP API（autoresearch-rs 内置，无需 AI 路由）
   → Top-2 draft claims 各有 3 篇相关论文结果
   → 证据填入 BARRIER_REPORT.json 的 candidates.evidence

3. hypothesis generation
   → draft-claims from state
   → 生成 3-5 候选假设 + 每条的 evidence 列表

4. feasibility scan
   → 对每个假设做 quick check
   → 标记 high/medium/low 可行性

5. return to loop
   → 输出 structured barrier report:
     - barrier: 原始问题
     - attempted: 已尝试的方案
     - candidates: 候选方案列表（含可行性评估）
     - recommended: 推荐优先级
   → loop runner 读取报告 → 选择候选 → 继续执行
```

**输出格式**：

```json
{
  "schema_version": "barrier-report-v1",
  "barrier": "原始问题描述",
  "context": {
    "loop_id": "loop-xxx",
    "run_id": "run-yyy",
    "action_id": "action-zzz",
    "consecutive_failures": 3
  },
  "attempted": ["方法A: 失败原因", "方法B: 失败原因"],
  "candidates": [
    {
      "id": "c1",
      "hypothesis": "候选假设",
      "confidence": "medium",
      "evidence": ["paper1: xxx", "paper2: yyy"],
      "expected_effort": "2h",
      "risk": "low: 已有成熟工具链"
    }
  ],
  "recommended": ["c1", "c3"],
  "generated_at": "ISO8601"
}
```

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

**Schema**：

```sql
CREATE TABLE exploration_logs (
    id TEXT PRIMARY KEY,
    direction TEXT NOT NULL,
    question TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT,
    tags TEXT,
    key_findings TEXT,
    status TEXT DEFAULT 'active'
        CHECK(status IN ('active', 'abandoned', 'concluded')),
    entry_point TEXT,             -- 'manual' | 'barrier_escalation' | 'loop'
    barrier_id TEXT               -- 关联的 barrier 报告 ID (nullable)
);

CREATE TABLE exploration_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_id TEXT NOT NULL REFERENCES exploration_logs(id),
    decision TEXT NOT NULL,
    rationale TEXT,
    alternatives TEXT,
    evidence TEXT,
    timestamp TEXT NOT NULL,
    outcome TEXT
);

CREATE TABLE exploration_insights (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_id TEXT NOT NULL REFERENCES exploration_logs(id),
    insight TEXT NOT NULL,
    confidence TEXT CHECK(confidence IN ('high','medium','low','speculative')),
    source TEXT,
    discovered_at TEXT NOT NULL,
    verified INTEGER DEFAULT 0,
    cross_refs TEXT               -- JSON array of other log IDs
);

CREATE TABLE barrier_reports (
    id TEXT PRIMARY KEY,
    loop_id TEXT,
    run_id TEXT,
    problem TEXT NOT NULL,
    attempted TEXT,
    candidates TEXT,
    recommended TEXT,
    resolution TEXT,              -- 'resolved' | 'abandoned' | 'pending'
    resolved_by TEXT,             -- candidate ID
    resolved_at TEXT,
    created_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE exploration_fts USING fts5(
    direction, question, key_findings,
    content='exploration_logs',
    content_rowid='rowid'
);
```

**操作命令**（通过 `cargo run -p research-harness --bin research-log` 调用，亦可从 `cargo run -p research-harness --bin autoresearch -- log:*` 桥接）：

```
cargo run -p research-harness --bin research-log -- record [--direction <name>] [--question <text>] [--entry-point <manual|barrier_escalation|loop>] [--barrier-id <id>]
                                                                 # 记录当前探索（文字层 + SQLite）
cargo run -p research-harness --bin research-log -- search <query> [--direction <dir>] [--status <active|abandoned|concluded>] [--limit <N>]
                                                                 # 跨方向 FTS5 检索
cargo run -p research-harness --bin research-log -- add-finding --entry-id <id> --kind <kind> --content <text> [--confidence <0-1>]
                                                                 # 记录新发现/insight（kind: insight | claim | risk | todo）
cargo run -p research-harness --bin research-log -- connect <log-id-a> <log-id-b> [--relation <text>] [--notes <text>]
                                                                 # 连接两个研究方向
cargo run -p research-harness --bin research-log -- barrier [--loop-id <id>]                         # 查询 barrier 报告列表（按 loop 或全部）
cargo run -p research-harness --bin research-log -- render <entry-id> [--write] [--output <path>]    # 渲染单篇日志为 Markdown
cargo run -p research-harness --bin research-log -- export --format <json|csv|obsidian> [--output <path>]
                                                                 # 导出日志集（JSON / CSV / Obsidian MD）
cargo run -p research-harness --bin research-log -- status                                           # 显示日志库状态（大小、条目数、WAL 模式）
cargo run -p research-harness --bin research-log -- consolidate                                      # 整理 activity log 文件

**Knowledge Graph 命令（§19.13）**：
cargo run -p research-harness --bin research-log -- neighbors <entry-id> [--relation] [--limit]       # 显示 entry 的直接连接
cargo run -p research-harness --bin research-log -- path --from <id> --to <id> [--max-depth]          # BFS 最短路径
cargo run -p research-harness --bin research-log -- viz [--entry-id] [--max-depth] [--format text|dot] # 知识图谱可视化（ASCII / Graphviz DOT）
cargo run -p research-harness --bin research-log -- graph-stats                                       # 全图统计（节点/边/密度/关系分布）
cargo run -p research-harness --bin research-log -- route --barrier-id <id> [--max-depth]              # Barrier 路径追溯
cargo run -p research-harness --bin research-log -- extract-entities <entry-id>                        # 自动提取 entry 中的知识实体
cargo run -p research-harness --bin research-log -- add-entity <name> [--kind] [--description]         # 手动添加知识实体
cargo run -p research-harness --bin research-log -- search-entities <query> [--limit]                  # FTS5 实体搜索
cargo run -p research-harness --bin research-log -- entry-entities <entry-id>                          # 显示 entry 关联的实体
cargo run -p research-harness --bin research-log -- hub-register [--path] [--name]                     # 注册到跨工作区 Hub
cargo run -p research-harness --bin research-log -- hub-search <query> [--limit]                       # 跨工作区 FTS5 搜索
cargo run -p research-harness --bin research-log -- hub-list                                           # 列出已注册的工作区
```

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

```
function detect_claim_drift(state):
    1. 比较 current_direction.original_question 与 active_hypothesis 的文本相似度
       - 若 < 0.5 → 标记为 structure drift
    2. 检查最近 N 条 run_history 是否在 falsifiable_prediction 的 perimeter 内
       - 若 ≥2 条超出 → 标记为 perimeter breach
    3. 检查最近 N 条 run_history 是否仍回答 original_question
       - 若描述的问题与 original_question 不同 → 标记为 question drift
    4. 聚合 drift_score = w1*structure + w2*perimeter + w3*question
    5. 若 drift_score > DRIFT_THRESHOLD → 输出警告 + 追加 deviation_log
```

#### 19.7.3 阈值与响应

| drift_score | 级别 | 响应 |
|-------------|------|------|
| < 0.3 | 正常 | 静默记录 |
| 0.3–0.6 | 注意 | 输出提示 + 追加 deviation_log |
| 0.6–0.8 | 警告 | 输出警告 + `deviation_warning_count++` |
| ≥ 0.8 或 `deviation_warning_count ≥ 3` | 强制 | 阻断执行，要求用户确认 |

#### 19.7.4 reflect 命令的 drift 输出格式

```
╔══════════════════════════════════════════╗
║         Claim Drift 检测报告             ║
╠══════════════════════════════════════════╣
║ 原始问题: XXX 方法能否提升 YYY 泛化性能  ║
║ 当前假设: ZZZ 条件下 accuracy 提升 5%    ║
║ ────────────────────────────────────────── ║
║ 结构偏移: 0.1 (正常)                      ║
║ 边界违例: 0.7 ⚠️ 最近运行超出 perimeter  ║
║ 问题漂移: 0.2 (正常)                      ║
║ ────────────────────────────────────────── ║
║ 综合评分: 0.45 (注意)                     ║
║ 累计警告: 2 / 3                           ║
║ ────────────────────────────────────────── ║
║ 建议: 检查实验是否仍然在 perimeter 内     ║
╚══════════════════════════════════════════╝
```

#### 19.7.5 循环中的 drift 检测

当 autoresearch 被 loop runner 通过 barrier escalation 调用时，drift 检测有额外规则：

```
loop barrier escalation → autoresearch init
    → 自动执行 drift 检测（以 barrier 描述为 original_question）
    → 若 drift_score > 0.6 → 追加到 barrier report 的 attempted 列表
    → 重新聚焦后再执行 external_research
```

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
> 找到候选方案后恢复循环。这是本文档新增的最重要的框架级变更。

#### 19.9.1 Loop 模式 Catalog — 新增

`docs/spec/loop-architecture.md` §9.2 开箱模式表应增加：

| Skill | Cadence | Safety | 产出 | 触发条件 |
|-------|---------|--------|------|---------|
| loop-research-barrier | 按需（on escalation） | L2 | barrier report + candidate list | consecutive_failures ≥ N |
| loop-hypothesis-test | 按配置 | L2 | hypothesis verification result | 新 hypothesis 就绪 |
| loop-literature-watch | 1w | L1 | new papers digest | cron |
| loop-claim-refresh | 1w-2w | L1 | drift detection report | cron |

#### 19.9.2 LOOP_REGISTRY.json 扩展

```json
{
  "loop_id": "my-experiment",
  "profile": "loop-auto",
  "research_enabled": true,            // 新字段：启用 research escalation
  "research": {
    "barrier_threshold": 3,             // 连续失败 N 次后触发
    "escalation_target": "autoresearch",
    "max_research_time_min": 30,        // 研究阶段最长耗时
    "auto_resume": true,                // 研究产出候选后自动恢复循环
    "require_human_approval": false      // 是否需要在候选方案上人工确认
  },
  "scope_based_safety": { ... },
  "cost_budget": { ... },
  "notification": { ... }
}
```

#### 19.9.3 状态真源与路径

```
artifacts/
├── loop/<loop-id>/                     ← loop 执行状态
│   ├── LOOP_RUN_STATE.json
│   ├── evidence/<action-id>/
│   └── reports/<run-id>.md
│
├── research-barrier/                   ← barrier 爆发的研究工件
│   └── <barrier-id>/
│       ├── BARRIER_REPORT.json         ← §19.4.3 格式
│       ├── research-state.yaml         ← autoresearch 状态
│       └── smoke-test-results.jsonl    ← barrier 触发时的快照
│
├── research-log/                        ← §19.5 分层日志
│   ├── INDEX.md
│   ├── YYYY-MM/
│   └── tags/
│
└── current/<task_id>/                  ← 现有 closeout 路径
    └── closeout/<task_id>.json
```

#### 19.9.4 执行流程（完整）

```
Loop Runner 执行 action
    → action FAIL × N (N ≡ barrier_threshold)
    → runner 检测到 consecutive_failures ≥ N
    → 检查 LOOP_REGISTRY.research_enabled
    →
    ├── enabled:
    │   → 构造 barrier description（含 loop_id + run_id + action_id + 失败上下文）
    │   → shell: autoresearch barrier <description>
    │   → autoresearch 执行 §19.4.3 barrier_escalation 流程
    │   → 产出 BARRIER_REPORT.json 到 artifacts/research-barrier/<barrier-id>/
    │   → runner 读取 BARRIER_REPORT.json
    │   → 若 auto_resume:
    │       → 从 recommended 选第一个
    │       → 构造新 action（safety 降一级）
    │       → 继续 DISPATCHING
    │   → 若 !auto_resume:
    │       → ESCALATED（等人）
    │
    └── disabled:
        → 按现有 escalation 路径（retry / escalate / record_and_skip）
```

#### 19.9.5 Loop 安全越界保护

当 research escalation 修改了 loop scope 外的文件时：
- **不阻止**（研究阶段可能需要修改配置/依赖）
- 但 barrier 报告中要明确标注 "out_of_scope_changes: [文件列表]"
- 恢复循环时，loop runner 对 out-of-scope 变更做 `git stash`（不销毁，仅隔离）

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

### 19.11 抗审查记录：已知框架性问题

> 本节记录对抗审查中发现的框架性问题，部分已在本规约中修复，
> 部分标注为已知 debt 待后续解决。

#### 19.11.1 ✅ 已修复（在本规约中）

| 问题 | 严重度 | 修复位置 |
|------|--------|---------|
| research-discovery `disable-model-invocation: true` → NL 路由命中后也无法触发 | P0 blocker | §19.2.1 → `false` |
| research-loadout `explicit_opt_in` → 用户不知道要手动激活 | ~~P1~~ ✅ 已修复 | §19.8 → moved to `default_loadouts` |
| research ↔ loop 完全隔离，无 barrier escalation | P0 | §19.9 新增 |
| NL 路由 trigger_hints 不覆盖日常用语（"查论文""找文献"） | P2 | §19.2.2 扩展 |
| autoresearch-rs 无 skill 入口 | P1 | §19.4 |
| paperplain MCP 未集成到 research-discovery | P2 | §19.2.5 |
| 科研结果无新鲜度校验 | P2 | §19.2.6 + §19.6 |
| claim drift 无防护 | P1 | §19.7 |

#### 19.11.2 ⚠️ 已知 debt（本规约不解决）

| 问题 | 严重度 | 原因 |
|------|--------|------|
| deep-research workflow（JS）和 research-discovery（MCP+HTTP）使用不同的数据平面 | P3 | 有意图差异（Web 调研 vs 学术文献），不合并，但需更好的入口指引 |
| autoresearch-rs 仍使用阻塞 HTTP（reqwest blocking），与异步框架不一致 | P2 | 重构成本高，功能不受影响 |
| loop architecture spec（v8）已实现，research-aware loop 已可用 | ~~P0~~ ✅ 已实现 | `core/loop-engine/` ~2666 LOC，`barrier_escalation()` 在 runner.rs 中通过 shell 调用 autoresearch CLI |
| BARRIER_REPORT.json 的 evidence 已由 Semantic Scholar + arXiv 自动填充 | ✅ 已修复 | candidates.evidence 现在包含论文标题+URL+作者 |
| `research-state.yaml` schema 版本为 4，但无 schema 验证 | P2 | 仅靠 `ensure_state_defaults` 做运行时修复 |
| `ROUTING_SIGNAL_MARKERS.json` 中无 barrier 相关信号定义 | ~~P2~~ ✅ 已修复 | 已新增 `barrier_escalation_signals` 组（17 个 marker） |
| NL_ROUTE_ADJUSTMENTS.json 被框架阻断直接修改 | ~~P2~~ ✅ 已标注 | barrier 相关 entries 已标注 `_signal_source` 引用 ROUTING_SIGNAL_MARKERS.json#barrier_escalation_signals |

#### 19.11.3 对抗审查结论

**负面**（当前框架的失败点）：
1. 交互式和自动循环是完全两个世界——但用户的工作流是连续的：写代码→遇到瓶颈→查文献→找到方案→继续写代码。框架没有反映这个连续体。
2. autoresearch-rs 的功能和定位完全正确，但**接入方式错误**——作为一个需要手动 `cargo run` 的 CLI 无法在日常科研中自然使用。它不是"另一个工具"，而是"如果放弃本地执行改用研究模式时的自然出口"。
3. research-discovery 的 `user-invocable: false` + `disable-model-invocation: true` 的组合效果是"完全无法被任何方式触发"——这是 self-acknowledged dead skill。

**正面**（这个方向的正确性）：
1. Barrier Escalation 是 autoresearch 和 loop 的自然交点——不是强行缝合，而是两者各自的内禀需要。
2. 分层日志（文字层 + SQLite FTS5）是科研的刚性需求——每天的探索需要被记录，但无需记录的探索也要能被检索。
3. Smoke Test 是数据驱动科研的前提——过期结果污染判断比没有结果更糟糕。

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

> 本节定义 Research Knowledge Graph（RKG）的功能契约：在扁平日志之上建立可查询的图结构，
> 包括条目关系遍历、知识实体提取、跨工作区索引、图可视化和 barrier 路径追溯。

#### 19.13.1 系统设计

```
连接存储（connections 表）
    │ 条目 A ──[supports]──→ 条目 B
    │ 条目 A ──[extends]───→ 条目 C
    ▼
图遍历引擎（graph.rs）
    │ load_full_graph / load_subgraph
    │ get_neighbors / find_path（BFS）
    │ bfs_traverse / dfs_traverse
    │ trace_barrier_route
    ▼
实体提取（extract.rs）
    │ 5 组 regex 模式：method / dataset / metric / model / tool
    │ entities 表 + entity_relations 表 + entities_fts
    ▼
跨工作区 Hub（hub.rs）
    │ ~/.claude/research-knowledge-hub.db
    │ 跨工作区搜索 + 统一索引
    ▼
可视化（viz 命令）
    │ ASCII box-drawing / Graphviz DOT
```

#### 19.13.2 核心数据结构

**connections 表扩展**（v2→v3 migration）：
- `weight REAL DEFAULT 1.0` — 边权重，影响图遍历优先级
- `confidence REAL` — 关系置信度 0.0-1.0

**entities 表**：
```sql
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,  -- method|dataset|theorem|metric|concept|tool|author|model|other
    description TEXT,
    metadata TEXT,        -- JSON
    created_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE entities_fts USING fts5(name, description, tokenize='unicode61');
```

**entity_relations 表**：
```sql
CREATE TABLE entity_relations (
    id INTEGER PRIMARY KEY,
    entity_id_a INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    entity_id_b INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,  -- uses|trains-on|evaluates|improves|depends-on|contradicts|is-a|part-of
    entry_id TEXT REFERENCES entries(id),
    confidence REAL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(entity_id_a, entity_id_b, relation)
);
```

**entry_entities 表**（条目 ↔ 实体关联）：
```sql
CREATE TABLE entry_entities (
    entry_id TEXT NOT NULL REFERENCES entries(id),
    entity_id INTEGER NOT NULL REFERENCES entities(id),
    role TEXT NOT NULL DEFAULT 'mentioned',  -- primary|mentioned|derived|compared
    PRIMARY KEY (entry_id, entity_id)
);
```

#### 19.13.3 图遍历算法

- **BFS 邻接加载**: 从 connections 表加载全部连接，构建内存邻接表（`HashMap<String, Vec<(neighbor, relation, weight, confidence)>>`）
- **最短路径**: BFS 无权最短路径，支持 `max_depth` 上限
- **子图抽取**: 从中心节点出发 BFS N 跳，过滤关联连接
- **Barrier 追溯**: 从 barrier_reports 表出发，找到关联条目，加载子图

#### 19.13.4 CLI 命令

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

#### 19.13.5 跨工作区 Hub

Hub 数据库位于 `~/.claude/research-knowledge-hub.db`，不绑定到单个 workspace。
schema 包含 `workspace_index`、`hub_entries`、`hub_entries_fts` 三张核心表。
用作"全局研究记忆"——跨项目搜索相关的工作、方法、线索。

#### 19.13.6 实体提取策略

无外部 NLP 依赖。5 组 hardcoded regex 覆盖量化金融/ML 常见 vocabulary：
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
| `spec.md` | 统一规约：7 层架构、路由与插件契约、运行时、安全生命周期、可观测性 |
| `core-policy` (代码) | Review gate 状态机、安全策略 |
| `runtime-core` (代码) | 运行时编排、closeout gate |

---

### 19.15 统一 Rust Crate 架构（`core/research-harness/`）

v7 引入 `core/research-harness/` crate，将散落在 `runtime-core`、`autoresearch-rs`、`research-log-rs`、`citation_tool_rs` 中的科研逻辑统一到单一 crate。SKILL.md 保留为用户前端，MCP tools 暴露 Rust API。

#### 模块结构

| 模块 | 职责 | 源 |
|------|------|-----|
| `search/` | 文献检索（Semantic Scholar, arXiv, paperplain MCP） | autoresearch-rs |
| `claims/` | Claim ledger 管理、drift 检测、ceiling 计算 | autoresearch-rs |
| `log/` | 研究活动日志（SQLite FTS5）、知识图谱、实体提取 | research-log-rs |
| `citation/` | 引用审计、BibTeX 渲染、DOI 验证 | citation_tool_rs |
| `review/` | 多轮对抗审稿编排、7 维度、收敛判定 | runtime-core/quality_gate.rs |
| `hooks/` | Prose/Adversarial/ActivityLog hooks | runtime-core hooks |
| `aigc/` | AIGC 检测（n-gram + burstiness + syntactic）、降重 | 新建 |
| `verification/` | 文献/统计/Prose QC/结构/形式验证 | scripts/verify/*.sh |
| `latex/` | LaTeX 数学公式解析与 SVG 渲染（基于 RaTeX） | RaTeX 开源项目 |
| `types.rs` | 共享类型（Finding, Claim, Paper, AigcResult...） | 新建 |

#### MCP Tools

通过 `host-projection` 的 `mcp_stdio_harness` 暴露：

- `research_review_dimensions` — 获取审稿维度 prompt + checklist
- `research_aigc_check` — AIGC 检测（0-100 评分 + 信号列表）
- `research_aigc_humanize` — AIGC 降重（句法改写/词汇替换）
- `research_latex_parse` — LaTeX 数学公式 AST 解析
- `research_latex_render_svg` — LaTeX 公式渲染为 SVG（支持内联/独立模式）

#### 依赖关系

```
research-harness
    ├── core-state (leaf crate, no cycle risk)
    ├── loop-engine (通用 loop 调度器)
    ├── ratex-lexer (LaTeX 词法分析器)
    ├── ratex-font (字体度量和符号表)
    └── workspace deps (anyhow, chrono, reqwest, rusqlite, serde, regex, ...)
```

**不依赖** `runtime-core` 或 `host-projection`，避免循环依赖。
`runtime-core` 可通过 trait object 或函数指针调用 `research-harness` 的 hook 接口。

#### 向后兼容

- `autoresearch-rs` / `research-log-rs` 保留为独立 binary（thin CLI wrapper 待完成）
- `host-projection` 的 hook 注册可渐进迁移为调用 `research_harness::hooks`
- 所有现有 MCP tool 名称不变，调用方无感知
| `spec.md` | 7 层模型中的 loop-auto 调度；桥接见 LOOP_REGISTRY.json + BARRIER_REPORT.json |
