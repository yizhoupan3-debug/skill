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
  version: '1.0.0'
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

- **用户需求模糊、需要澄清问题 → 先走 `$deepinterview`**（deepinterview 是「还不知道要什么」，smoke 是「已经有东西要测」）
- **需要深度根因排查 → 走 `$systematic-debugging`**（smoke 是广度覆盖，devugging 是深度追踪）
- **需要完整 code review（审代码质量/安全） → 走 `$code-review-deep`**（smoke 是验证功能行为，不是审代码）
- **需要完整文献调研或学术实验设计 → 走 `$research`**（smoke 只做快速 probe，不做系统性调查）
- **只有模糊兴趣/想法/方向 → 先问清楚再测**（smoke 要求有具体的被测对象）

## Workflow

### 场景 A：内部部件诊断

1. **分解部件** —— 梳理目标系统的部件边界，条目化输出部件清单
2. **设计 smoke** —— 对每个部件设计一个最简可执行验证（最小输入 → 预期输出），标注可观测指标
3. **逐件 smoke** —— 逐个执行：单独运行、去掉该部件运行、对比基线
4. **整理结论** —— 每部件的贡献/增益/损害量化结果 → 部件影响矩阵
5. **建议** —— 哪些部件关键、哪些可有可无、哪些拖累、哪些可替换

输出格式：
```
| 部件 | 增益贡献 | 损害/成本 | 可替换性 | 建议 |
|------|---------|---------|---------|------|
| A    | +30%    | 5ms     | 高       | 保留 |
| B    | -2%     | 0ms     | 中       | 可砍 |
```

### 场景 B：外部方案评估

1. **锁定候选** —— 明确外部方案名称/版本/接口
2. **现有基线** —— 当前部件的关键指标（功能集、性能、维护成本等）
3. **差距分析** —— 候选方案覆盖现有功能的程度、功能缺口、额外能力
4. **快速试跑** —— 如果可行，对候选方案做最小可行执行验证
5. **替换成本评估** —— 集成难度/API 适配/迁移成本/维护风险
6. **结论** —— 推荐替换/不推荐/有条件替换（条件是什么）

输出格式：
```
| 维度         | 现有 | 候选 | 差距 | 影响 |
|--------------|------|------|------|------|
| 功能覆盖率   | 80%  | 60%  | -20% | blocker |
| 性能(吞吐)   | 100  | 120  | +20% | 正收益 |
| 集成成本     | -    | 3d   | -    | 中   |
```

## Automation (MCP tools)

此 skill 的自动化由 `core/research-harness` crate 的 3 个 MCP 工具驱动：

| 工具 | 作用 | 对应场景 |
|------|------|----------|
| `research_smoke` | 运行可执行模板，注入参数为环境变量，解析 stdout JSON | 单次实验/基础 smoke |
| `research_ablation` | 跑基线 + 逐个去部件，返回贡献矩阵（Δ、增益、损害、推荐） | 场景A — 部件诊断 |
| `research_evaluate` | 对比 baseline vs candidate 方案的功能覆盖/性能/集成成本/推荐 | 场景B — 方案评估 |

### 场景A 自动编排

```
1. agent("分解部件", schema=ComponentsSchema)
   → 输出: [{name, description, test_command, metrics}]

2. agent("为每个部件编写 ablation 模板脚本", schema=ScriptSchema)
   → 输出: baseline.sh + 每个部件的 ablation 脚本
   → 模板脚本读取 EXPERIMENT_SMOKE_ABLATION_REMOVED 环境变量来判断哪个部件被去除

3. research_ablation(template, baseline_params, components, metrics)
   → 输出: {baseline, components: [{name, deltas, contribution_score, damage_score, recommendation}], summary}

4. agent("解释 ablation 结果", schema=ContributionMatrix)
   → 输出: 决策建议（哪些保留/优化/移除）
```

### 场景B 自动编排

```
1. agent("提取当前方案基线", schema=BaselineSchema)
   → 输出: {name, template, params, capabilities}

2. web_search / gh / crates.io 等方式获取候选方案信息
   → 输出: {name, template, params, capabilities, dimensions}

3. research_evaluate(baseline, candidate, dimensions)
   → 输出: {metrics, coverage, integration_cost, verdict}

4. agent("解释评估结果", schema=EvaluationReport)
   → 输出: 推荐结论
```

### 脚本模板约定

模板脚本（放在 `templates/` 下）必须遵守以下契约：

- **输出**：stdout 末行必须是 JSON 对象 `{"metric_name": numeric_value, ...}`
- **Ablation 感知**：检查 `EXPERIMENT_SMOKE_ABLATION_REMOVED` 环境变量
  - 未设置或空 → 全量运行（baseline）
  - 设置为部件名 → 去掉该部件后运行
- **参数注入**：所有参数通过 `EXPERIMENT_<UPPERCASE_KEY>` 环境变量传入
- **退出码**：0 表示运行成功（即使结果不佳），非 0 表示运行失败

示例模板（shell）：
```bash
#!/bin/bash
# templates/benchmark.sh
# 感知 ablation：
if [ -n "$EXPERIMENT_SMOKE_ABLATION_REMOVED" ]; then
    echo "Ablation mode: $EXPERIMENT_SMOKE_ABLATION_REMOVED removed" >&2
fi

# 参数从环境变量读取
LR="${EXPERIMENT_LR:-0.01}"
BS="${EXPERIMENT_BS:-32}"

# 运行实验...
result=$(python -c "print('{\"accuracy\": 0.85, \"latency_ms\": 42}')")

# 输出结果（末行 JSON）
echo "$result"
```

### CLI 接口

`research_smoke` 引擎也通过 `autoresearch` CLI 暴露：

```bash
cargo run -p research-harness --bin autoresearch -- smoke-test \
    --template benchmark.sh \
    --params '[{"lr": "0.01", "bs": "32"}]' \
    --concurrency 4 \
    --timeout-ms 60000
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
