---
last_verified: "2026-06-27"
---

# 路由引擎架构

## 1. 双管道设计

路由系统由两条独立的评分管道组成，分别处理 skill 路由和 tool 路由：

| 管道 | Crate | 评级步数 | 用途 |
|------|-------|---------|------|
| Skill Routing | `core/routing-engine` | 16-step | 将用户自然语言查询路由到最匹配的 skill（SKILL.md） |
| Tool Routing | `core/tool-routing-engine` | 8-step | 将用户自然语言查询路由到最匹配的 MCP 工具 |

两条管道共享 `routing-core` 中的基础原语（n-gram 余弦相似度、trigram Jaccard 模糊匹配），但评分逻辑完全独立。Skill routing 处理 owner/overlay/gate 竞争，而 tool routing 使用单 dispatch_domain 的扁平评分。

## 2. Skill Routing 16-step 评分管道

定义在 `core/routing-engine/src/route/scoring.rs` 的 `score_route_candidate()` 函数中。每个候选 skill 记录按以下 16 步独立评分：

| 步骤 | 名称 | 函数 | 说明 |
|------|------|------|------|
| 1 | NL pre-framework-alias | `apply_nl_pre_framework_alias_rules` | NL 路由调整：pre-framework-alias 规则 |
| 2 | Agent-swarm signals | `score_agent_swarm_signals` | 多 agent/worker/并行执行信号检测与加分 |
| 3 | Framework alias suppression | `check_framework_alias_suppression` | 检查是否需要 suppression（仅显式 alias 调用） |
| 4 | NL post-framework-alias | `apply_nl_post_framework_alias_rules` | NL 路由调整：post-framework-alias 规则 |
| 5 | Design-md signals | `score_design_md_signals` | 设计合约/design token 信号检测 |
| 6 | Framework alias explicit boost | (inline) | 显式 alias 调用加固定分 |
| 7 | Gate/name-token/trigger-hint | `score_gate_name_token_signals` | Gate 短语匹配、精确 skill 名匹配、name token 匹配、trigger hint 匹配 |
| 7.5 | N-gram semantic similarity | `score_ngram_signal` | 基于 character n-gram 的语义相似度加分 |
| 8 | Metadata/keyword/alias | `score_metadata_trigger_signals` | 元数据正触发词、keyword token、alias token 匹配 |
| 9 | Session-start signals | `score_session_start_signals` | session_start required/preferred 加分 + code-review-deep boost |
| 10 | Gate owner boost | (inline) | owner_lower == "gate" 时加固定分 |
| 11 | Visual-review logic | `score_visual_review_signals` | 视觉证据上下文检测与弱匹配抑制 |
| 12 | Do-not-use penalty | (inline) | 匹配 do_not_use_tokens 时扣分 |
| 13 | Paper-workbench boost | `score_paper_workbench_signals` | 论文校对/修改意图检测 |
| 14 | CodeGraph boost | (inline) | codegraph 索引上下文加分 |
| 15 | Overlay suppression | (inline) | overlay-only 记录乘以抑制因子 |
| 16 | RouteCandidate 构造 | (return) | 返回最终 `RouteCandidate { score, reasons, matched_token_count }` |

### 2.1  Owner 选择（`pick_owner`）

评分完成后的 owner 选择逻辑 (`scoring.rs::pick_owner`)：

1. **Agent-swarm gate 优先**：如果 gate 候选分数 >= `agent_swarm_candidate_threshold` 且非 plan-mode/systematic-debug 上下文
2. **Top owner 阈值**：最高 owner 分数 >= `top_owner_score_threshold`
3. **Gate-before-owner**：gate 候选分数 >= `gate_before_owner_threshold` 且 >= top owner 分数
4. **Layer-aware 竞争**：按 L0→L4 层排序，每层最高分超过层阈值者胜出
5. **Fallback**：按 layer → score → priority → slug 综合排序

### 2.2 Overlay 选择（`pick_overlay`）

允许一个额外的 overlay skill 与主 owner 同时加载。匹配条件：
- 记录 owner 为 overlay
- 查询中显式匹配 overlay 的 slug_lower 或 trigger_hints
- 不会叠加自身（`filter_overlay_self`）

特殊 case：`behavior:framework_review_overlay` flag + 框架 review 上下文 → 自动叠加 code-review-deep。

### 2.3 Fuzzy 救援

当精确匹配管道无结果或低于阈值时，触发 `fuzzy_rescue_primary_record`：
- 使用 `core/routing-core` 的 trigram Jaccard 相似度
- 应用 layer penalty（L0: 0, L1: -0.02, L2: -0.05, L3: -0.08, L4: -0.12）
- 最低相似度阈值：`FUZZY_MIN_SIMILARITY`
- CI gate skill 标记了 `behavior:ci_gate_fuzzy_rescue_excluded` 时跳过
- Overlay 记录不参与 fuzzy rescue

## 3. N-gram 语义相似度引擎

定义在 `core/routing-engine/src/route/ngram.rs`。

### 架构

```rust
pub(crate) struct NgramCache {
    query_uni: HashMap<String, usize>,  // unigram (1-char) 频次向量
    query_bi: HashMap<String, usize>,  // bigram (2-char) 频次向量
    query_tri: HashMap<String, usize>, // trigram (3-char) 频次向量
}
```

每次 `route_task()` 调用创建一个 `NgramCache` 实例，跨所有 skill 记录复用。

### 相似度计算

对于任意目标字符串 `target`：
1. 提取目标字符串的 unigram/bigram/trigram 频次向量（通过 `routing_core::fuzzy::character_ngrams`）
2. 分别计算三个粒度的余弦相似度（通过 `routing_core::fuzzy::cosine_similarity`）
3. 加权合成：`0.5 × cos_uni + 0.3 × cos_bi + 0.2 × cos_tri`

### 信号集成

`score_ngram_signal()` 将查询与 skill 记录的每个 trigger_hint 进行 n-gram 相似度比较，取最大值并钳制到 `[0.0, ngram_similarity_max]`。此信号设计用于捕捉 token-based 关键词匹配无法覆盖的语义重叠，尤其是 CJK 跨语言查询。

### 与 trigram fuzzy 的关系

| 维度 | N-gram 语义相似度 | Trigram Jaccard fuzzy |
|------|------------------|----------------------|
| 位置 | Step 7.5（评分管道内） | 精确匹配管道失败后的救援 |
| 粒型 | 1+2+3-char 加权余弦 | 3-char Jaccard |
| 实现 | `routing-engine/src/route/ngram.rs` | `routing-core/src/fuzzy.rs` |
| 用途 | 加分信号，与 keyword/gate 叠加 | 独立 fallback 替代方案 |

## 4. 可观测性

### Routing Logger

`core/routing-engine/src/route/routing_logger.rs`

每次 `route_task()` 调用记录一条 JSON line 到 `logs/routing/routing_audit.ndjson`：

```json
{
  "ts": "2026-06-26T12:00:00Z",
  "query": "...",
  "session_id": "...",
  "selected_skill": "...",
  "score": 85.0,
  "layer": "L2",
  "fuzzy_match": false,
  "matched_token_count": 3,
  "overlay_skill": "code-review-deep",
  "top_3_reasons": ["Exact skill name matched: ...", ...]
}
```

- 10 MB 自动轮转
- 线程安全（`Mutex<BufWriter<File>>`）
- 日志目录通过 `FRAMEWORK_ROOT` env var 解析

### Zero-match Collector

`core/routing-engine/src/route/zero_match_collector.rs`

所有分数 ≤ 0.0 的查询（无任何 skill 匹配）被记录到 `logs/routing/zero_matches.ndjson`。数据驱动缺失覆盖分析和 trigger hint 规划。

- 10 MB 自动轮转
- 线程安全
- 与 routing_logger 共享 `logs/routing/` 目录

### Hook 消费

路由决策通过 `framework-kernel::runtime_hooks` 的 `telemetry.route_decision` 回调被 L5+ 层只读消费。路由引擎自身不依赖消费方：

```
routing-engine ──route_decision event──→ framework_kernel::runtime_hooks::hooks().telemetry.route_decision
                                           ↓
                                     L5+ 只读消费
```

## 5. 黄金数据集与评估框架

### 黄金数据集

`tests/routing_eval_cases.json`（92 条，schema: `routing-eval-cases-v1`）

每条 case 包含：
- `id`：唯一标识
- `category`：`should-trigger` / `should-not-trigger` / `wrong-owner-near-miss` / `gate-vs-owner-conflict` / `fuzzy-rescue` / `overlay` / `stop-token` / `framework-alias`
- `task`：用户查询文本
- `expected_owner`：期望选中的 skill slug
- `expected_overlay`：期望选中的 overlay skill slug
- `forbidden_owners`：禁止选中的 owner 列表
- `expected_layer`：期望的层
- `route_context`：期望的 route context payload
- `first_turn`：是否首轮

### 评估框架

`core/routing-engine/src/route/eval.rs` 中的 `evaluate_routing_cases()` 函数：
1. 加载 JSON case 文件
2. 对每条 case 执行 `route_task()`
3. 比较实际结果与期望（owner、overlay、layer、forbidden_owner）
4. 生成 `RoutingEvalReportPayload` 包含 metrics（trigger_hit、trigger_miss、overtrigger、owner_correct、overlay_correct）

集成测试在 `core/routing-engine/tests/eval_harness_contracts.rs` 中。

### 评分快照测试

`scoring_common_queries`、`scoring_empty_query`、`scoring_chinese_query` 等 insta snapshot 测试覆盖常见查询的评分输出，确保重构不改变路由行为。

## 6. 路由-Hook 衔接

### 数据流

```
用户查询
    │
    ▼
route_task() ──→ skill records × scoring_pipeline ──→ RouteDecision
    │                                                      │
    │  routing_logger (routing_audit.ndjson)                │
    │  zero_match_collector (zero_matches.ndjson)           │
    │                                                       │
    ▼                                                       ▼
RuntimeCoreHooks.emit_route_decision() ──→ L5+ 消费（telemetry, observability）
    │
    ▼
manifest_retry_logic ──→ should_retry_with_manifest / should_accept_manifest_fallback
```

- 路由引擎**完全独立决策**，不依赖外部 hook 注册状态
- `framework-kernel::runtime_hooks` 只提供只读的 telemetry 回调
- Manifest retry 逻辑（`should_retry_with_manifest`）是路由后处理，不在评分管道内

### 关键函数入口

| 函数 | 位置 | 职责 |
|------|------|------|
| `route_task()` | `routing.rs:336` | 主路由入口：评分 → 选择 → overlay → 日志 |
| `search_skills()` | `routing.rs:199` | 搜索模式：评分 + top-k 返回 |
| `route_tool()` | `tool-routing-engine/routing.rs:25` | 工具路由入口 |
| `should_retry_with_manifest()` | `routing.rs:775` | 判断是否需要 manifest fallback |
| `should_accept_manifest_fallback()` | `routing.rs:975` | 判断 manifest fallback 决策是否可接受 |
| `evaluate_routing_cases()` | `eval.rs:34` | 评估框架 |

## 7. 相关文件索引

| 文件 | 说明 |
|------|------|
| `core/routing-engine/src/route/routing.rs` | 主路由函数、manifest fallback 逻辑 |
| `core/routing-engine/src/route/scoring.rs` | 16-step 评分管道、owner/overlay 选择 |
| `core/routing-engine/src/route/ngram.rs` | N-gram 语义相似度引擎 |
| `core/routing-engine/src/route/scoring_config.rs` | 评分权重配置 |
| `core/routing-engine/src/route/routing_logger.rs` | JSON lines 审计日志 |
| `core/routing-engine/src/route/zero_match_collector.rs` | 零匹配查询收集 |
| `core/routing-engine/src/route/eval.rs` | 评估框架 |
| `core/routing-engine/src/route/fuzzy.rs` | Trigram Jaccard 模糊匹配 |
| `core/tool-routing-engine/src/routing.rs` | 工具路由 8-step 管道 |
| `core/framework-kernel/src/runtime_hooks.rs` | Hook 注册表（telemetry.route_decision） |
| `tests/routing_eval_cases.json` | 92 条黄金评估数据 |
| `core/routing-engine/tests/eval_harness_contracts.rs` | 评估集成测试 |
