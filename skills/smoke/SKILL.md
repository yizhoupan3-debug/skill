---
name: smoke
description: 快速烟雾测试 — 内部部件诊断（每部件贡献/增益/损害）+ 外部方案快速评估（是否值得替换现有部件）
framework_contracts:
  consumes_execution_items: false
  consumes_findings: false
  emits_execution_items: false
  emits_findings: true
  emits_verification_results: true
framework_roles:
  - reviewer
  - evaluator
metadata:
  platforms:
  - supported
  tags:
  - smoke
  - testing
  - ablation
  - evaluation
  - diagnosis
  version: '1.0.3'
network_access: conditional
scene: research
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: 快速烟雾测试 — 内部部件诊断 + 外部方案评估
trigger_hints:
- smoke test
- smoke-test
- 烟雾测试
- 快速验证
- 部件诊断
- 部件贡献
- 组件验证
- 去掉会怎样
- ablation test
- 实际增益
- 每个部件
- 逐部件
- 测一下各个部件
- 快速评估方案
- 外部方案评估
- 是否值得替换
- 替换代价
- 现有方案对比
- 换方案
- 方案评估
- 快速 prototyping
- 评估第三方
- PR 快速验收
- 改动影响
- 改动验证
- 功能完整性
- 冒烟测试
- 冒烟
- 快测
- 各个组件
- 筛一下
- 初步验证
- 初步测试
---

## 路由三问（优先判断）

进入 smoke 之前，先过这三个问题：

| # | 问题 | 是 → 做什么 | 否 → 转什么 |
|---|------|------------|-------------|
| 1 | 用户有具体的可执行对象（代码/文件/命令/API）？ | 继续 | → `$deepinterview`（需求未收敛） |
| 2 | 用户想要的是"测一下每块的贡献"或"评估这个方案值不值"？ | 继续 | → 无模糊需求则不处理 |
| 3 | 用户说的"smoke"是科研实验参数可运行性探测（templates 目录）？ | → `$research-workspace` | 继续用本 skill |

> **核⼼判别**：smoke = "已经有东西要测"，deepinterview = "还不知道要什么"

## When to use

### 场景 A：内部部件诊断
用户已有实现（函数库/模块集合/系统），需要**一一测试每个部件的实际贡献/增益/损害**：
- "帮我 smoke 一下这个实现，看看每块到底有多大用"
- "逐部件 ablation，去掉每个组件看结果变化"
- "测一下各个模块的独立贡献"
- "验证每个步骤是否真的有增益"
- "哪些代码可以砍掉而不影响"

### 场景 B：外部方案评估
用户面临一个外部方案/库/工具，需要快速判断**是否值得替换现有部件**：
- "这个新库和我们现有的比怎么样"
- "评估一下这个方案替换的代价和收益"
- "快速测一下这个外部方案能不能用"
- "第三方组件的功能缺口在哪里"
- "现有方案 vs 候选方案 quick eval"

### 通用触发
- 任何需要「先跑一轮看看」「快速验证」「初步测试」「筛一遍」的场景
- PR 或改动的快速验收（功能完整性验证）
- 对已有系统做探查性测试

## Do not use

- **用户需求模糊、需求未收敛 → 先 `$deepinterview`**
- **深度根因排查（未知故障、偶发崩溃）→ `$systematic-debugging`**（smoke 是广度，debugging 是深度追踪）
- **完整 code review（代码质量/安全审查）→ `$code-review-deep`**（smoke 测行为而非审代码）
- **完整文献调研/学术实验 → `$research`**（smoke 只做快速 probe）
- **用户说"有一个想法/思路"尚无具体实现 → 先问清楚再测**

## LLM ⼯作流

### 场景 A：内部部件诊断

```
step 1: 分解部件
───────────────
agent("梳理目标系统的部件边界", schema = {部件清单})
→ 输出: [{name, description, how_to_isolate, metrics}]
→ how_to_isolate 描述如何去掉该部件运行
   (例如: 注释 import、交换实现、加 --no-feature flag、改 config)
→ metrics 是每个部件的关键可观测指标
   (如: latency_ms, accuracy, throughput, memory_mb)

step 2: 为每个部件生成 ablation 模板脚本
───────────────
→ 从当前代码库提取测试入口
→ 为 baseline(全量) 和每个部件(去掉该部件) 各编写 one-shot 测试脚本
→ 放在 templates/ 目录下
→ 模板脚本必须:
   • 检查 $EXPERIMENT_SMOKE_ABLATION_REMOVED
     - 空/未设置 → baseline 模式（全量运行）
     - 有值 → 去掉该部件后运行
   • 末行 stdout 输出 JSON: {"metric": numeric_value, ...}
   • exit 0 表示运行成功（即使结果差），非 0 表示失败

step 3: 运行 ablation
───────────────
research_ablation({
  template: "benchmark.sh",
  baseline_params: {同 step 1 定义的参数},
  components: [{name, description, ablation_params?}],
  metrics: ["acc", "latency"]  // 关注的指标，空则自动检测
})

→ 输出(例子):
{
  "baseline": { "result": {"acc": 0.95, "latency": 50} },
  "components": [{
    "component": "module_a",
    "deltas": {"acc_delta": -0.30, "latency_delta": 10},
    "contribution_score": 0.68,
    "damage_score": 0.05,
    "recommendation": "critical"
  }, ...],
  "summary": {"critical":2, "retain":1, "optimizable":3, "removable":1}
}

step 4: 解释结果并输出决策建议
───────────────
→ 根据 contribution_score / damage_score 判断每部件命运
→ 输出最终建议: 保留/优化/移除/替换
```

### 场景 B：外部方案评估

```
step 1: 锁定当前基线
───────────────
agent("提取当前方案的信息")
→ 输出: {name, template, params, capabilities: [功能点列表], dimensions: [{name, higher_is_better}]}

step 2: 获取候选方案信息
───────────────
→ web_search / gh / crates.io / npm / pypi 等方式收集
→ 也跑一遍它的 smoke 模板
→ 输出: {name, template, params, capabilities: [功能点列表]}

step 3: 运行评估
───────────────
research_evaluate({
  baseline: {name, template, params, capabilities},
  candidate: {name, template, params, capabilities},
  dimensions: [{name: "throughput", higher_is_better: true, weight: 2}, ...]
})

→ 输出(例子):
{
  "dimensions": [{"name":"throughput", "baseline":100, "candidate":200, "delta":100, "winner":"candidate"}],
  "coverage": {"shared":["auth"], "baseline_only":["reporting"], "candidate_only":["streaming"], "gap_score":0.5},
  "integration_cost": {"person_days": 2.5, "risk": "medium"},
  "verdict": {"recommendation": "conditional", "confidence": 0.45, "reasoning": [...]}
}

step 4: 输出推荐
───────────────
→ replace: 值得替换。信任条件: confidence > 0.6
→ conditional: 有条件替换。列明缺什么（gap），满足后可升级为 replace
→ reject: 不值得。列明为什么（性能倒挂/缺口太大/集成成本过高）
```

## MCP 工具参考

| 工具 | 作用 | 关键参数 | 输出 |
|------|------|---------|------|
| `research_smoke` | 运行模板注入参数 | template, params, concurrency, timeout_ms | `{experiments: [{run_id, exit_code, result, wall_time_ms}]}` |
| `research_ablation` | 部件贡献矩阵 | template, baseline_params, components, metrics | `{baseline, components[{name, deltas, contribution_score, damage_score, recommendation}], summary}` |
| `research_evaluate` | 方案对比 | baseline, candidate, dimensions | `{metrics, coverage, integration_cost, verdict}` |

## 结果解读指南

### Ablation 矩阵（场景 A）

| 输出字段 | 含义 | 阈值 |
|----------|------|------|
| `contribution_score` (0~1) | 该部件对系统的正面贡献 | >0.6=critical, >0.4=retain, >0.2=optimizable, 余=removable |
| `damage_score` (0~1) | 该部件引入的负面影响（性能开销等） | >0.5=removable |
| `delta` | 去掉部件后该指标的变化量 | 负数(对 HigherIsBetter)=该部件有正面贡献 |
| `recommendation` | 推荐意见 | critical/retain/optimizable/removable/insufficient_data |

**解读逻辑**：
- **critical**（贡献大/损害小）：核心组件，不能动。任何修改需要最高优先级测试
- **retain**（贡献中/损害小）：有价值的部件，保留但可优化
- **optimizable**（贡献小/损害小）：边际部件。可以考虑重构或合并
- **removable**（贡献小/损害大或贡献极小）：可以直接移除，或者用外部方案替换

### 评估报告（场景 B）

| 输出字段 | 含义 | 决策影响 |
|----------|------|----------|
| `verdict.recommendation` | replace/conditional/reject | 最终建议 |
| `verdict.confidence` | 置信度 0~1 | >0.6=可信 |
| `coverage.gap_score` | baseline 功能在 candidate 中的缺失比例 | gap 越高，替换成本越大 |
| `integration_cost.risk` | low/medium/high | 影响是否值得替换 |

### 两个场景如何切换

```
用户请求
  ├── 有实现/代码要看每块贡献 → 场景 A (内部诊断)
  │     └── 用 research_ablation
  │
  ├── 有现有方案和候选方案要对比 → 场景 B (外部评估)
  │     └── 用 research_evaluate
  │
  └── 就在代码库内对比两个实现（一个 baseline 一个 candidate）
        → 适用场景 B 的逻辑，但 template 指向同一个本地文件
        → 场景 B 本质上就是"对比两个点"——不一定是外部方案
```

## Constraints

- **快**：每个 smoke 点应在 60 秒内完成验证（跑不快的说明不是 smoke，是完整测试）
- **浅**：不追求覆盖边界和异常，只验证核心路径是否通
- **可见**：每个 smoke 都有明确的 pass/fail/stats 输出
- **可重复**：design smoke 时保证输入确定、环境确定、输出可对比
- **不取代**：smoke 不取代单元测试/集成测试/性能测试 — 它是快速探查，不是质量门

## Hard constraints

- 用户必须提供或指明确切的可执行对象（代码/文件/命令/API）—— 空口问「这个做不做得好」不属于 smoke
- 每个 smoke 点必须设计最简输入输出，不做冗余验证
- 部件诊断时必须单独测量去部件后的影响（ablation），不只是列功能清单
- 外部评估时必须评估替换成本（集成难度 + 维护风险），不只比功能
- 同一个会话内如果确认需要深度分析，handoff 给对应的 deep 技能（`$systematic-debugging` / `$code-review-deep` / `$research`）
