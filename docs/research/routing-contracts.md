---
parent: ../research-harness.md
---

# Research Routing Contracts

> 本文档是科研 Harness 路由契约的真源，由 `docs/research-harness.md` §19.2–19.3 拆分而来。
> 系统总览与拓扑见 [research-harness.md](../research-harness.md)。

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

