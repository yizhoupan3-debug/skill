---
parent: docs/spec.md
version: unified-v7
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
    → tools/autoresearch-rs/ (Rust CLI，跨 skill 共享)
    → tools/research-log-rs/ (分层日志，§19.5)
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

### 19.2 research-discovery 路由契约

#### 19.2.1 路由元数据

```yaml
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: false        # 保持隐藏，不开放 /research-discovery
disable-model-invocation: false  # NL 路由命中后允许模型触发
```

#### 19.2.2 trigger_hints（三类场景全覆盖）

> **关键规则**：trigger_hints 必须覆盖用户的**日常自然用语**，而非仅限科研术语。
> NL 路由基于 `text_matches_phrase`（token 级子串匹配），
> 单 token 的 CJK 短语（如"查论文""找文献"）评分低于多 token 精准短语，
> 因此需要**数量冗余**来弥补单 token 匹配权重不足。

| 场景 | 示例 hints | 匹配难度 |
|------|-----------|---------|
| 科研方向调研 | `调研方向` `文献综述` `研究方向` `知识地图` `主题深挖` `研究综述` `相关定理` `理论背景` `数学背景` | 易（多 token > 2字） |
| 论文/学术搜索 | `查论文` `找文献` `有哪些论文` `学术调研` `学术搜索` `最新进展` `sota` `state of the art` `paper search` `literature` | 中（部分为短 token） |
| 性质探索 | `未知性质` `性质不清楚` `类比` `该用什么理论` `用什么数学` `深度调研这个科研方向` `深调研` | 易（多 token） |
| hard barrier 升级 | `突破不了` `硬指标` `无法突破` `瓶颈` `卡住了` `roadblock` `stuck` `这个方向走不通` | 中（需与 ROUTING_SIGNAL_MARKERS.json 协同） |

#### 19.2.3 alias_tokens

```
["论文", "文献", "学术", "调研", "方向", "landscape", "sota", "survey", "gap", "突破", "瓶颈", "走不通", "stuck"]
```

#### 19.2.4 路由评分保障

- `trigger_hint_per_match: 20.0`（每个 hint 匹配 +20 分）
- `layer_threshold_L2_L3: 14.0`（1 个 hint 匹配即可过阈值）
- 在 `NL_ROUTE_ADJUSTMENTS.json` 中注册 boost 规则（识别"瓶颈""卡住"等 barrier 信号时 boost research-discovery）
- 在 `host_projection_narrative.json` 中保留 `research_harness_paragraph`

#### 19.2.5 external_research lane — 学术检索契约

**first-class**: `paperplain` MCP 工具（`fetch_paper`、`find_paper_by_title`、`search_research`）处理结构化学术查询。

**fallback**: 当 MCP 不可用时，手动 HTTP 调用 `references/academic-sources.md` 定义的五个源。

**paperplain MCP 工具与学术源映射**：

| MCP 工具 | 等价源 | 适用场景 |
|----------|--------|---------|
| `search_research(domain=cs)` | arXiv + Semantic Scholar | CS/AI 方向搜索 |
| `search_research(domain=health)` | PubMed + Semantic Scholar | 生物医学方向搜索 |
| `search_research(domain=general)` | 全部三个源 | 跨学科搜索 |
| `fetch_paper(paper_id)` | arXiv/PubMed/Semantic Scholar | 单篇论文元数据获取（DOI / arXiv ID / PubMed ID） |
| `find_paper_by_title(title)` | Semantic Scholar | 按标题模糊匹配 |

**Multi-source fan-out 模板**（用于深度调研）：

```
parallel:
  1. paperplain search_research(domain=cs, query="{topic}")       # arXiv+S2 广度
  2. paperplain search_research(domain=general, query="{topic}")  # 跨学科
  3. paperplain find_paper_by_title("{known important paper}")     # 已知论文验证
deduplicate → synthesize → evidence map
```

#### 19.2.6 新鲜度守卫

每条 `external_research` 结果必须携带 `freshness` 元数据：

```json
{
  "source": "arxiv",
  "query": "...",
  "freshness": {
    "fetch_time": "ISO8601",
    "oldest_result_date": "ISO8601",
    "newest_result_date": "ISO8601",
    "coverage_window_days": 42,
    "stale": false,
    "stale_reason": null
  }
}
```

**stale 判定规则**：
- `newest_result_date < now - 180d` → `stale: true`, `reason: "no recent results"`
- `coverage_window_days > 730` 且查询包含年度关键词 → `stale: true`, `reason: "query needs year filter"`

**契约**：stale 结果不得作为强证据使用；须标注限制或重查。

#### 19.2.7 Lane 手递关系

| Lane | 入口 | 出口 |
|------|------|------|
| `research_question` | 研究方向界定 | → `external_research` 或 `$research-execution` |
| `external_research` | 文献/学术搜索 | → 证据地图 |
| `math_background_inquiry` | 数学理论地图 | → `$math-derivation` 或 `formal-verification` |
| `paper_handoff` | 成为手稿级 | → `$paper-workbench`（含 `language_register` + prose-chain-contract） |

#### 19.2.8 Barrier Escalation 入口

当 NL 输入包含 hard barrier 信号（"突破不了""卡住了""瓶颈""stuck""roadblock""这个方向走不通"）
且路由引擎检测到用户处于实施/实验上下文时，**优先路由到 research-discovery** 而非 execution：

```
检测到 barrier 信号 + 实施上下文
    → routing engine 给予 research-discovery +20 boost（NL_ROUTE_ADJUSTMENTS.json）
    → research-discovery 初始化研究方向界定
    → 产出证据地图 + 候选方案
    → 手递回 execution 或 autoresearch
```

---

### 19.3 research-execution 路由契约

```yaml
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: false
disable-model-invocation: false
```

**trigger_hints**：详见 SKILL.md，覆盖实验设计 + 数学建模 + 代码核查三类场景。

**Lane**：
- `experiment_design`: 变量/控制/消融/基线/指标/样本量/停止条件
- `math_verification`: 假设/推导见证/定理依赖/验证器选项
- `math_modeling`: 变量/方程/闭合/无量纲组/状态图
- `code_verification`: 实现审计/测试/确定性复现/基准命令
- `reproducibility`: 环境/数据版本/种子/配置/制品追踪

**研究层次晋级规则**：

```
discovery 阶段 (research-question / literature / theory)
    │ 发现不明确 → 回 research-discovery
    │
    ▼
execution 阶段 (experiment / math / code)
    │ 需要新文献 → 回 research-discovery
    │ 遇到硬指标突破不了 → research-discovery (barrier escalation, §19.2.8)
    │
    ▼
paper 阶段 → $paper-workbench
```

---

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

所有 lane 通过 `cargo run -p autoresearch-rs -- <subcommand>` 调用 `tools/autoresearch-rs/` CLI。

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

**操作命令**（通过 `research-log-rs` CLI 调用，亦可从 `autoresearch log:*` 桥接）：

```
research-log-rs record [--direction <name>] [--question <text>] [--entry-point <manual|barrier_escalation|loop>] [--barrier-id <id>]
                                                                 # 记录当前探索（文字层 + SQLite）
research-log-rs search <query> [--direction <dir>] [--status <active|abandoned|concluded>] [--limit <N>]
                                                                 # 跨方向 FTS5 检索
research-log-rs add-finding --entry-id <id> --kind <kind> --content <text> [--confidence <0-1>]
                                                                 # 记录新发现/insight（kind: insight | claim | risk | todo）
research-log-rs connect <log-id-a> <log-id-b> [--relation <text>] [--notes <text>]
                                                                 # 连接两个研究方向
research-log-rs barrier [--loop-id <id>]                         # 查询 barrier 报告列表（按 loop 或全部）
research-log-rs render <entry-id> [--write] [--output <path>]    # 渲染单篇日志为 Markdown
research-log-rs export --format <json|csv|obsidian> [--output <path>]
                                                                 # 导出日志集（JSON / CSV / Obsidian MD）
research-log-rs status                                           # 显示日志库状态（大小、条目数、WAL 模式）
research-log-rs consolidate                                      # 整理 activity log 文件
```

> **注意**：`log:route <barrier-id>`（从 barrier 追溯完整研究路径）尚未在 CLI 中实现。

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
| research-loadout `explicit_opt_in` → 用户不知道要手动激活 | P1 | §19.8 → `default` |
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
| loop architecture spec（v8）尚未实现，§19.9 的 research-aware loop 依赖于它 | P0 blocker for loop | loop-engine 代码未编写，当前无法端到端测试（已在 spec-loop-architecture.md 首行标注） |
| BARRIER_REPORT.json 的 evidence 已由 Semantic Scholar + arXiv 自动填充 | ✅ 已修复 | candidates.evidence 现在包含论文标题+URL+作者 |
| `research-state.yaml` schema 版本为 4，但无 schema 验证 | P2 | 仅靠 `ensure_state_defaults` 做运行时修复 |
| `ROUTING_SIGNAL_MARKERS.json` 中无 barrier 相关信号定义 | P2 | 需要在配置文件中增加 "barrier_escalation" 信号组 |
| NL_ROUTE_ADJUSTMENTS.json 被框架阻断直接修改 | P2 | 需通过 Rust host-entrypoint sync 或 routing path 修改 |

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
| `docs/spec/research-harness.md`（本文件） | 框架维护者 | 所有科研 Harness 变更 |
| `skills/research-discovery/SKILL.md` | Skill 作者 | 路由契约/lane/barrier 入口变更 |
| `skills/research-discovery/references/academic-sources.md` | Skill 作者 | 学术源新增/更新 |
| `skills/research-execution/SKILL.md` | Skill 作者 | 路由契约/lane 变更 |
| `skills/autoresearch/SKILL.md` | Skill 作者 | workspace lane / barrier escalation 变更 |
| `tools/autoresearch-rs/src/` | Rust 开发者 | CLI 功能变更（含 barrier 子命令） |
| `tools/research-log-rs/src/`（已实现） | Rust 开发者 | 日志系统 + FTS5 查询 |
| `artifacts/research-log/smoke-tests.json` | 研究员 | 新查询/方向/barrier 注册 |
| `artifacts/research-barrier/` | loop runner + autoresearch | barrier escalation 自动写入 |
| `configs/framework/LOOP_REGISTRY.json` | 框架维护者 | loop research 模式注册 |
| `configs/framework/ROUTING_SIGNAL_MARKERS.json` | 框架维护者 | barrier 信号定义 |
| `configs/framework/NL_ROUTE_ADJUSTMENTS.json` | 框架维护者 | barrier boost 规则 |
| `SKILL_LOADOUTS.json` | 框架维护者 | loadout 变更 |
| `SKILL_ROUTING_RUNTIME.json` | 框架维护者 | 路由注册变更 |
| `docs/spec/loop-architecture.md` | 框架维护者 | loop patterns catalog 中 research 模式 |

### 19.13 与框架其他规约的关系

| 关联文档 | 关系 |
|---------|------|
| [routing-plugin.md](routing-plugin.md) | §8 路由与插件契约 — research skill 的 L2 路由遵循此规约 |
| [runtime-subsystems.md](runtime-subsystems.md) | §9 ResearchMode 运行时在此定义 |
| [auxiliary.md](auxiliary.md) | §14.4 harness_context_signals 中数学信号与 research-discovery `math_background_inquiry` lane 共享 |
| [observability-testing.md](observability-testing.md) | §17 测试契约包含 autoresearch-rs 覆盖统计；本规约 §19.10 为其科研领域特化 |
| [security-lifecycle.md](security-lifecycle.md) | §12 closeout 为 research-execution 的验证 gate |
| [loop-architecture.md](loop-architecture.md) | §19.9 research-aware loop 模式在此注册，本规约与 loop 架构通过 LOOP_REGISTRY.json 和 BARRIER_REPORT.json 桥接 |
