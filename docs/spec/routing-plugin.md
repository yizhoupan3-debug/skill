---
parent: docs/spec.md
version: unified-v8
---

## 8. 路由与插件契约

### 8.1 Skill 注册（当前实现）

**真源**：`skills/SKILL_ROUTING_RUNTIME.json`（列索引格式）

- `load_records_from_runtime()` — 纯配置加载
- `SKILL_ROUTING_METADATA.json` — 运行时元数据补丁
- `filter_records_for_host()` — 按 host_platforms 过滤
  - `SKILL_MANIFEST.json` 中 `host_platforms` 支持通配符令牌：`"supported"` / `"all-hosts"` 展开为 `host_targets.supported` 全集（规范化逻辑见 `tests/host_platforms.rs`）
- 可插拔性：**4.0/5**

### 8.2 路由评分（当前实现，待废弃）

- `route_task()` → `score_route_candidate()`（~1019 行硬编码，可插拔性 **2.0/5** — 下游依赖制约大幅重构）
- `signals.rs`：单一文件（~1520 行），含路由信号检测（design_artifact/devtools/markers/orchestration/paper 等类别）
- `nl_route_adjustments.rs`：NL suppress/boost 调整（746 行 Rust 适配层 + 61 条手工规则）

**已知维护痛点（触发本变更）：**
- 每新增一个 skill 需同步修改 4-5 个文件（signals.rs + NL_ROUTE_ADJUSTMENTS.json + scoring.rs 可能 + 路由表 + 分层文档）
- `signals.rs` 的 68 个信号定义全靠人工维护 keyword 列表，embedding drift 和语义重叠导致规则膨胀
- 61 条 NL 调整规则形成隐式依赖网络，修改一条可能影响多条 skill 的 routing surface
- 评分逻辑高度耦合（`scoring.rs` 1016 行单一函数），无法独立测试或替换单个评分维度
- 上述合计约 6916 行路由引擎代码，维护成本随 skill 数量超线性增长

### 8.3 路由引擎模块（当前实现）

| 子模块 | 功能 | 状态 |
|--------|------|------|
| `routing.rs` | 主路由逻辑 + manifest fallback | ⏳ 8.4 将取代 |
| `scoring.rs` | 候选评分（boost/suppress/overlay） | ⏳ 8.4 将取代 |
| `records.rs` | 记录加载 + 缓存（mtime-based OnceLock+RwLock） | ✅ 保留 |
| `policy.rs` | 路由策略载荷 | ✅ 保留 |
| `text.rs` | 文本规范化 + 分词 | ⏳ 8.4 将取代 |
| `aliases.rs` | 框架 alias 检测 | ✅ 保留 |
| `nl_route_adjustments.rs` | NL suppress/boost 调整 | ⏳ 8.4 将取代 |
| `signals.rs` | 关键字信号检测（1520 行） | 🗑 8.4 废弃 |
| `signal_cache.rs` | 信号缓存 | 🗑 8.4 废弃 |

---

## 8.4 下一代路由架构：语义路由核（Semantic Route Core, SRC）

> **设计目标**：消除手工规则维护，以 embedding 语义聚类 + 置信度引导替代 6916 行手工编码规则。维护焦点从「写路由规则」转向「维护示例语句 + 日志驱动的被动进化」。
>
> 个人使用场景：无并发、低对抗风险、可直接切换（git 回滚兜底）。以下设计排除多租户权限、并发安全、灰度放量等企业级顾虑。

### 8.4.1 架构总览

```
用户查询
    │
    ├─→ [Alias/Gate pre-check] ──匹配──→ 直接路由（不走 embedding）
    │    ($delegate, $paper-workbench, gate hints…)
    │
    ▼
┌──────────────────────────────────────────────────────┐
│  8.4.2 Intent Embedding Classifier (IEC)             │
│  candle + local ONNX model → query embedding         │
│  → 余弦相似度 vs 各 skill centroid → (slug, score)    │
│  → 按得分排序                                         │
│  → 如果冷启动（无 centroid）→ 降级到旧 signals.rs       │
└────────────────────┬─────────────────────────────────┘
                     │ Top-N (slug, score)
                     ▼
┌──────────────────────────────────────────────────────┐
│  8.4.3 Confidence-Guided Router (CGR)                │
│  score > τ_boost → boost skill                       │
│  score > τ_auto  → 正常路由                           │
│  τ_gate < score < τ_auto → Parity Gate               │
│    → hybrid: embedding + signals.rs 联合判决          │
│    → 仍不一致 → 选更高 confidence 的                  │
│  score < τ_zero → suppress                           │
└────────────────────┬─────────────────────────────────┘
                     │ (评分调整指令 + 置信度)
                     ▼
┌──────────────────────────────────────────────────────┐
│  8.4.4 Plugin Scoring Engine (PSE)                   │
│  含 TokenBudgetScore — 按 skill 成本动态调权          │
│  昂贵 skill（deep-research）需更高置信度              │
│  廉价 skill（gitx）可低阈值触发                       │
└────────────────────┬─────────────────────────────────┘
                     │ (最终排序)
                     ▼
              路由决策 → log → 用户使用
                                    │
                                    ▼ (用户修正?)
                         日志系统 (JSONL, ~200B/行)
                                    │
                                    ▼ (离线)
                         定期 centroid 重计算
                         从日志中挖掘新 utterances
```

### 8.4.2 Intent Embedding Classifier (IEC)

#### 实现策略（个人场景，不做 async）

```rust
struct IntentClass {
    slug: String,
    utterances: Vec<String>,           // 示例语句
    centroid: Vec<f32>,                // embedding 均值（非 Option — 冷启动时为空 Vec）
    threshold_boost: f32,              // 每个 skill 独立阈值（默认值引用全局，可覆盖）
    token_cost: u32,                   // 该 skill 典型单次调用的 token 消耗（见 8.4.4）
}

// 全局注册表
struct IntentRegistry {
    classes: Vec<IntentClass>,
    embedding_model: Box<dyn EmbeddingModel>,
    eval_cases: Vec<EvalCase>,          // 冷备 eval 用例
    fallback_signals: FallbackSignals,  // 指向旧 signals.rs（降级路径）
}
```

**Embedding 方案**：`candle` + ONNX 格式本地模型。

| 模型 | 维度 | 推理时间（M3 Max） | 多语言 |
|------|------|-------------------|--------|
| `all-MiniLM-L6-v2` | 384 | ~5ms | ❌ 英文 only |
| `intfloat/multilingual-e5-small` | 384 | ~8ms | ✅ 中英混排（推荐） |

- 本地推理：无需 API 调用，无网络延迟，同步 `fn embed()` 直接返回
- ScorePlugin 同步签名 `fn score(&self, ...) -> f32` 直接调用
- 冷启动时 centroid 为空 Vec → skip embedding → fallback 到 `signals.rs` 的 keyword 信号

#### 冷启动（解决了 C3/C13）

```
冷启动 ─→ 首次查询
             │
             ├─ 取 IntentRegistry 中的 eval_cases（从旧路由日志提取的种子）
             │   → 对每条 eval_case 做 keyword 匹配（复用 signals.rs 函数）
             │   → 收集命中结果作为 utterances 种子
             │
             ├─ centroid = mean(utterances_embedding)
             │
             └─ 正常运行
```

关键：冷启动**不等于 100% parity gate**。旧 `signals.rs` 函数（当前仍在代码库中）作为冷启动期间的 fallback。IEC 的 embed 路径与 keyword 路径同时运行，keyword 路径持续提供路由决策，直到 IEC 积累足够 utterances 后接管。

#### Utterance 管理（个人场景 = 无投毒风险，去掉了 C11）

| 层 | 来源 | 维护方式 | 上限 |
|---|------|----------|------|
| L1 种子 | SKILL.md + trigger_hints → LLM 生成 | 创建 skill 时一次 | 3-10 条 |
| L2 积累 | 路由 log 中记录的成功路由（用户未重路由） | 离线脚本追加（人工审核） | 每 skill ≤ 50 条，超限裁剪最远距离 |

个人场景下无投毒风险（用户自己不会攻击自己），但仍设上限防止无限膨胀。裁剪策略：保留距 centroid 最近的 50 条 utterance，丢弃 outlier（距离 > 2σ）。

### 8.4.3 Confidence-Guided Router (CGR)

#### 阈值

阈值**绑定 embedding 模型**（解决了 M5），不同模型有独立的预设值：

```json
{
  "embedding_model": "intfloat/multilingual-e5-small",
  "thresholds": {
    "τ_boost": 0.78,
    "τ_auto": 0.65,
    "τ_gate": 0.48,
    "τ_zero": 0.25
  }
}
```

| 模型 | τ_boost | τ_auto | τ_gate | τ_zero | 说明 |
|------|---------|--------|--------|--------|------|
| e5-small (384d) | 0.78 | 0.65 | 0.48 | 0.25 | multilingual，中文友好 |
| MiniLM-L6 (384d) | 0.82 | 0.70 | 0.55 | 0.35 | 英文 only，相似度分布偏集中 |
| text-embedding-3-small (API, 1536d) | 0.85 | 0.75 | 0.60 | 0.40 | 高维，分布偏集中 |

注意：这些是初始值。运行一段时间后从日志统计中自动校准（见 8.4.6）。

#### Parity Gate（重写，去掉了 C3/C6/C7/C8）

```
输入：embedding top-3 得分 / 当前 signals.rs 评分 top-3

情况 A：embed top-1 与 signals top-1 一致
    → 采纳，置信度 = max(embed_score, 0.9)

情况 B：embed top-1 ≠ signals top-1，但 embed top-2 中包含 signals top-1
    → 如果 embed_score(top-1) - embed_score(top-2) < 0.05（边界模糊）
    → 采纳 signals 的 top-1，触发 L2 积累（这条查询应加入目标 skill 的 utterances）
    → 否则采纳 embed top-1

情况 C：embed 全部得分 < τ_gate
    → 完全依赖 signals.rs
    → 触发告警（该 utterance 未被任何 centroid 覆盖）

情况 D：冷启动（centroid 全空）
    → 完全依赖 signals.rs
    → 自动从 signals 的命中模式生成种子 utterances（见 8.4.2 冷启动）
```

**不需要 LLM Judge**（个人场景下无必要引入额外 LLM 调用和延迟）。Current `signals.rs` 已经是一个可工作的 fallback。

#### 规则残余

仅保留**必须显式硬编码的约束**：

| 类别 | 保留原因 | 实现 |
|------|----------|------|
| Alias 前缀 `$xxx` | 显式指定不可被 embedding 覆盖 | routing.rs 入口处 pre-check |
| Special Gate（delegation 等） | Gate 优先级高于意图分类 | routing.rs 入口处 pre-check |
| 宿主过滤（host_platforms） | 配置驱动，不可由语义推断 | records.rs 中按 host 过滤 |

对比当前 61 条规则 → **0 条 NL 调整规则**。CGR 不再需要独立的规则引擎。

### 8.4.4 Token 预算感知评分（新增维度）

#### 设计动机

不同 skill 的 token 消耗差异巨大：`gitx` 一次调用 ~500 tok，`deep-research` 一次调用 ~500K tok。路由决策应当考虑这个成本差异——同等置信度下优先路由到廉价 skill，昂贵 skill 需要更高置信度。

#### Skill 成本登记

每个 IntentClass 声明典型 token 成本（从实际使用统计校准）：

```rust
struct TokenCost {
    typical_input: u32,     // 典型 prompt tokens
    typical_output: u32,    // 典型生成 tokens
    total_tokens: u32,      // typical_input + typical_output
    cost_tier: CostTier,    // cheap / medium / expensive
}

enum CostTier {
    Cheap,      // < 5K tok    (gitx, gh-address-comments, doc, pdf)
    Medium,     // 5K-50K tok  (code-review, paper-workbench, diagramming)
    Expensive,  // > 50K tok   (deep-research, autoresearch, algo-trading)
}
```

初始成本表（从路由日志统计校准，见 8.4.5）：

| Skill | 成本等级 | 典型 total tok | 说明 |
|-------|---------|---------------|------|
| gitx | cheap | 800 | git 操作，少量对话 |
| gh-address-comments | cheap | 1500 | PR 摘要 + 评论 |
| doc / pdf / spreadsheets | cheap | 2000-4000 | artifact 处理 |
| code-review | medium | 8000 | diff 分析 |
| paper-workbench | medium | 15000 | 多轮 prose 编辑 |
| diagramming | medium | 5000 | 图生成 |
| research-discovery | expensive | 30000 | 多源搜索 |
| deep-research | expensive | 200000+ | 多阶段、多 agent 并行 |
| autoresearch | expensive | 100000+ | 研究工作区执行 |

#### TokenBudgetScore Plugin

```rust
impl ScorePlugin for TokenBudgetScore {
    fn score(&self, candidate: &RouteCandidate, context: &RouteContext) -> f32 {
        let cost = &candidate.intent.token_cost;
        let confidence = context.semantic_confidence;  // 来自 IEC
        
        match cost.cost_tier {
            // 廉价 skill：低阈值触发
            Cheap => confidence * 1.0 + 10.0,  // 偏好廉价 skill
            // 中等：正常
            Medium => confidence * 1.0,
            // 昂贵：需要高置信度，否则打折
            Expensive => {
                if confidence > 0.80 {
                    confidence * 1.0
                } else {
                    confidence * 0.5 - 20.0  // 显著抑制
                }
            }
        }
    }
}
```

效果：
- 同样 0.75 置信度，`gitx`（cheap）得分 85 → 优先上；`deep-research`（expensive）得分 17.5 → 除非用户明确表达研究意图
- `τ_auto` 之上的昂贵 skill 仍可正常路由
- 用户可通过手动重路由纠正（log 记录纠正，后续校准成本表）

#### Token 上下文维度的长期价值

- 随着使用量增长，从日志聚合出**每 skill 实际 token 消耗统计**（非预设值）
- 当 token budget 紧张时（`remaining < threshold`），自动提升 cheap skill 优先级
- 为未来「根据当前上下文剩余自动路由」打下基础

### 8.4.5 路由日志系统（新增 — 极简存储，训练数据源）

#### 设计原则

- **一行一条路由决策**，append-only，不需要索引
- **定位 debug + 训练数据源**，非审计合规
- **个人场景下一年数据 < 10MB**（假设每天 100 次路由）

#### 日志格式

每路由决策一条 JSON line，追加写入 `<project>/data/routing_log.jsonl`：

```jsonl
{"ts":"2026-06-18T10:30:00+08","q":"帮我 review 这个 PR","sid":"abc123","embed":"e5-small","cands":[{"s":"gh-address-comments","sc":0.82,"t":1200},{"s":"code-review-deep","sc":0.55,"t":8000}],"chosen":"gh-address-comments","sig_top1":"gh-address-comments","pg":"direct","lat":12,"fixed":""}
{"ts":"2026-06-18T10:31:00+08","q":"调研因果推断新方法","sid":"abc123","embed":"e5-small","cands":[{"s":"deep-research","sc":0.88,"t":200000},{"s":"research-discovery","sc":0.45,"t":30000}],"chosen":"deep-research","sig_top1":"research-discovery","pg":"hybrid","lat":15,"fixed":"research-discovery"}
```

| 字段 | 含义 | 类型 | 字节 |
|------|------|------|------|
| `ts` | ISO 时间戳 | string | ~25 |
| `q` | 原始查询（截断 200 字） | string | ~50 avg |
| `sid` | session 简写（前 6 位 hex，自增） | string | 7 |
| `embed` | embedding 模型名 | string | ~20 |
| `cands` | top-3 候选 `(slug, score, token_cost)` | array | ~100 |
| `chosen` | 最终路由到的 skill | string | ~20 |
| `sig_top1` | signals.rs 的 top-1（parity gate 参考） | string | ~20 |
| `pg` | parity gate 模式: direct/hybrid/fallback/startup | string | ~10 |
| `lat` | 路由决策延迟（ms） | number | 3 |
| `fixed` | 用户手动修正后的 skill（空=默认正确） | string | ~20 |

**一行约 200-300 字节。100 次/天 × 365 天 ≈ 7-10MB/年。**

#### 日志生命周期

```
routing_log.jsonl      ← 当前文件（append-only）
routing_log.2026-06.jsonl  ← 按月归档（自动切割）
```

- 当前文件达到 10MB → 自动重命名为 `routing_log.YYYY-MM.jsonl`，新建当前文件
- 保留最近 12 个月日志，更早的可手动清理
- 如果需要，用 `gzip` 可压至 ~15% 体积

#### 日志驱动进化

```
日志 → 离线脚本 → 聚合统计          → 校准 TokenCost（实际 token 消耗）
                → 提取成功路由       → 追加 L2 utterances（高置信度 + 无修正）
                → 提取用户修正       → 关键训练数据（最有价值的信号）
                → 计算 per-skill 平均置信度、gate 率 → 校准阈值
```

**核心回路**：用户修正 → log → 离线重算 centroid → 下次路由更准。不需要独立的训练管道。

### 8.4.6 长期维护与进化（替代迁移计划）

#### 接缝点（Seams）

- 旧 `signals.rs` 的 `has_*` 函数**不删除**，作为 parity gate fallback 保留。个人场景下 ~1520 行代码不构成维护负担
- 新代码以独立模块嵌入 `core/routing-engine/src/route/`，与旧代码并行
- 切换开关：`configs/framework/SRC_CONFIG.json` 中 `"routing_mode": "legacy" | "hybrid" | "src"`

#### 打通路径

1. **立即**：IEC 模块落地（candle + e5-small），输出到日志，`routing_mode = "legacy"`（只观察不干预）
2. **几周后**：查看日志，确认 IEC top-1 与 signals.rs top-1 一致率 > 80% → 切 `routing_mode = "hybrid"`
3. **自然过渡**：hybrid 模式下 CGR parity gate 起作用。随着 utterances 积累，consistency 提升，gate 率下降。无硬切换日期
4. **稳定后**：`routing_mode = "src"`，旧 `signals.rs` 不再做路由决策，但保留为 parit gate fallback

#### 进化回路

```
[每 N 次路由或每月]
  │
  ├─ 从 routing_log.jsonl 读取本月所有路由
  ├─ 聚合 per-skill：
  │    - 平均置信度
  │    - gate 率（pg ≠ direct 的比例）
  │    - 用户修正率（fixed ≠ "" 的比例）
  │    - 实际 token 消耗
  ├─ 阈值校准：
  │    - 如果某 skill 修正率 > 10% → 提高 τ_auto（更保守）
  │    - 如果某 skill 三个月无修正 → 降低 τ_auto（可信任）
  │    - 如果某 skill 永远在 cheap 池但实际 token > 5K → 升级到 medium
  ├─ Centroid 重算：
  │    - 从日志提取置信度 > 0.85 且无修正的路由 → 作为新 utterances
  │    - 重新 embed → 新 centroid
  │    - 在 eval_cases 上运行回归：新 centroid 的 top-1 与旧 centroid 一致率
  │    - 一致率 > 95% → 自动替换
  │    - 否则 → 打印 diff，手动决定
  └─ 写入 SRC_CONFIG.json（阈值更新 + centroid 快照）
```

**低成本执行**：Shell 脚本 `scripts/src-evolve.sh`，无服务器，无数据库。跑一次约 30 秒（全量 re-embed 45 skill × 50 utterances）。

#### 回滚

- Centroid 快照存储在 `configs/framework/centroid_snapshots/YYYY-MM-DD.json`（Float32 数组，45 × 384 × 50 ≈ 3.3MB 压缩前）
- `src-evolve.sh 回滚 --date 2026-06-01` → 恢复 centroid 快照 + 对应 config
- git revert 恢复代码级回滚

### 8.4.7 维护成本对比

| 维度 | 当前架构 (v7) | 新架构 (SRC v8) | 改善 |
|------|--------------|-----------------|------|
| 信号定义 | 1520 行 Rust 手工 keyword | 保留为 fallback + 种子生成器 | 0（不删除） |
| 调整规则 | 61 条 JSON + 746 行 Rust 适配 | 0 条（CGR 替代） | 100% |
| 评分逻辑 | 1016 行单一函数 | 5 个插件 × ~50 行 + TokenBudget | ~75% |
| 新 skill 接入 | 改 4-5 个文件 | 写 3-10 条 utterances + 确认 TokenCost | ~80% |
| 路由延迟 | ~1ms | ~15ms（本地 embedding） | 可接受 |
| embedding 模型 | 无 | 本地 candle ONNX | 0 API 成本 |
| 日志 | 无 | JSONL, ~200B/行, ~10MB/年 | 新增（debug + 训练） |
| 进化机制 | 无 | 日志驱动的离线校准脚本 | 新增 |
| 总代码量 | ~6916 行 | ~3500 行（含旧 fallback） | ~50% |

### 8.4.8 已知风险与缓解

| 风险 | 缓解 |
|------|------|
| e5-small 多语言 embedding 精度不够区分 45 个 skill | Cold start 阶段验证；不够则换 e5-large（768d, ~15ms） |
| 新代码 bug 导致路由错误 | `routing_mode = "legacy"` 阶段只观察不决策；旧 `signals.rs` 始终保留 fallback |
| Utterance 累积导致 centroid 漂移 | 每 skill ≤ 50 条裁剪 + 回归一致率 > 95% 自动替换 |
| 日志文件积累过多 | 10MB 自动归档 + 12 个月保留 + gzip |
| 个人使用某些 edge case 漏覆盖 | 用户手动重路由 → 日志记录 → 下次 evolve 校准 |

---

