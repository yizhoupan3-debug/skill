---
description: 先用本地证据起草可执行计划，产出可验收 todo；默认轻量，高风险时 audit。
metadata:
  platforms:
  - supported
  tags:
  - plan
  - gate
  - closeout
  version: '2.0.0'
name: plan-mode
scene: general
risk: low
routing_gate: none
routing_layer: L1
routing_owner: owner
routing_priority: P2
session_start: preferred
source: project
trigger_hints:
- Plan 模式
- 策划文档闸门
- 可验收 todo
- 计划对照实际
- 纯调研
- 调研计划
- plan-mode
- research-only plan
---
# plan-mode

把「写计划」当成**证据先行、可验收、可对照收口**的产物，而不是一次性 prose。默认不要把小任务拖进审计级流程；只有跨模块、高风险、用户明确要求时，才升级到完整 audit plan。

## When to use

- 用户要在 **Plan 模式**下先把范围、风险、验证路径钉死，再允许大规模改动。
- 用户提到 **策划文档闸门**、**可验收 todo**，或明确要求审计级计划。
- **每轮对话开始**：任务看起来像「先出高质量计划/蓝图」且后续实现依赖该计划的验收标准。
- 用户要 **纯调研 plan**：只产出深度、多角度 **只读**调研 todo，**不包含**实现/改代码/改测试。

## Do not use

- 用户只要直接实现、明确禁止前置规划。
- 单一极小改动（单文件几行）且计划成本明显高于收益 → 直接最小 delta + 验证。

## Plan depth

| 档位 | 适用 | 最小要求 |
|------|------|----------|
| **轻量 plan** | 小中型、低风险、单 owner 任务 | 五行证据（Goal / Non-goals / 已读证据 / 最大风险 / 首选验证）+ 可验收 todo |
| **标准 plan** | 需要落盘、跨文件或需对照实现的任务 | todos 写四元组；末条做计划 vs 实际对照 |
| **audit plan** | 跨模块、高风险、用户明确要求审计划/深度 review | 标准 plan + 可选 review-only findings + 更严格证据门槛 |

## 流程

1. **本地证据先进计划**：在写结构化计划前，完成域内必要的深读、检索或代码定位；计划应收敛已有证据，而不是用计划代替定位结论。
2. **Todo 必须可验收**：每条 todo 写全 **四元组**（动作 / 范围 / 完成定义 / 验证手段）。
3. **可选 review 只找问题**：仅当用户明确要求 review plan / 审计划时，review lane 只读计划与证据，输出 findings / risks / missing tests；不改代码、不自动修复。
4. **收口**：审批且实现通过后做计划 vs 实际逐项对照。

## Todo 可执行性

### 必备四元组（每条 todo 自检）

| 维度 | 要求 |
|------|------|
| **动作** | 动词 + 对象 |
| **范围** | 主要路径 1–3 个；忌「全仓优化」式模糊 |
| **完成定义（Done when）** | 可勾选、可客观判定 |
| **验证手段（Verify）** | 完整命令或明确人工步骤 |

可选第五行 **Non-goals**：一行写明本步**不**改什么。

### 单条模板

```text
[id] <动词> <对象> @ <path1>[, <path2>]
Done: <条件1>; <条件2>
Verify: <命令 或 人工检查步骤>
Non-goals: <可选>
```

### 拆分依赖

- **一条 todo ≈ 一个可合并 PR 级结果**（或更小）。出现「且 / 然后 / 另外」时拆成多条。
- **分支 / 多选一**：为每条分支写独立 todo + **仅当** / **Blocked by: <todo-id>**。忌单条「执行整条 P0 链」。
