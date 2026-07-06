# Smoke ↔ 相邻 Skill 重叠边界

本文件记录 smoke（烟雾测试）与框架既有 skill 的重叠边界与路由规则。

---

## 1. smoke ↔ deepinterview (L2, L1)

### 重叠区域

用户说「帮我 smoke 一下」「快速验证」时，如果语境不够具体，可能误路由到 deepinterview（需求澄清）。

### 核心差异

| 维度 | smoke | deepinterview |
|------|-------|---------------|
| **用户已有什么？** | 具体实现/方案/代码/API | 只有模糊想法或需求 |
| **目标是什么？** | 测出来结果的贡献/可行性 | 弄清楚用户到底要什么 |
| **动作** | 执行、验证、对比、量化 | 提问、澄清、收敛、定义 |
| **输出** | 部件影响矩阵 / 方案评估表 | 清晰的需求描述 / findings |
| **节奏** | 快 — 逐件测试 60s/项 | 慢 — 单轮单问收敛 |
| **典型话术** | "测一下各个模块" "验证这个改动" | "先问清楚" "需求不清晰" |

### 决策树

```
用户说"帮我 smoke 一下"
  ├── 用户有具体代码/方案/实现对象 → smoke
  │     (直接开工，不需要澄清定义)
  │
  ├── 用户只有模糊想法或场景说明 → deepinterview
  │     (先问清楚再考虑是否需要 smoke)
  │
  └── 用户有对象但不确定 smoke 什么 → smoke
        (skill 引导用户缩小范围/聚焦关键部件)
```

### 规则

| 条件 | Owner | 原因 |
|------|-------|------|
| 用户提及具体文件名/函数名/命令/URL | smoke | 被测对象明确 |
| 用户说"有一个思路/想法/方向" | deepinterview | 需求未收敛 |
| 用户有完整实现但"不确定对不对" | smoke → 如果 smoke 发现根因未知问题 → systematic-debugging | 先 smoke 探查，发现坑再深挖 |
| 用户只有需求文档/PRD | deepinterview | 还没到测的阶段 |

### 链式使用

```
deepinterview（需求澄清）→ smoke（快速测候选方案）
smoke（发现根因不明异常）→ systematic-debugging（深度调查）
smoke（发现代码质量问题）→ code-review-deep（代码审查）
```

---

## 2. smoke ↔ research-workspace (两者均为 scene=research)

### 重叠背景

两个 skill 同属 `scene=research`，且 `research-workspace` 的 trigger_hints 也包含 "smoke test"。但两者的 smoke test 内涵不同：

| 维度 | smoke | research-workspace |
|------|-------|---------------------|
| **本意** | 部件贡献诊断 + 方案价值评估 | 科研实验快速可运行性探测 |
| **workflow** | 一一分解部件 → ablation → 量化贡献矩阵 | 用 `templates/` + `params` 跑实验模板 |
| **后端** | 纯 skill 定义，用当前会话环境执行验证 | `core/research-harness/smoke.rs` 引擎 |
| **输出** | 部件贡献表 / 方案对比表 | 实验 JSONL 结果 |
| **用户** | "各个模块贡献多大""能不能用" | "这个参数配置能不能跑通" |

### 路由分辨

两者 trigger_hints 重叠 "smoke test" 一词。区分方式：

```
用户说"smoke test"
  ├── 上下文涉及具体实验模板/参数/配置 → research-workspace
  │     ("跑一下 ablation 实验模板")
  │
  ├── 上下文涉及部件贡献/实现/代码/方案评估 → smoke
  │     ("帮我 smoke 这个实现，每块的贡献")
  │
  └── 上下文不足以区分 → smoke
        (smoke 定义含 handoff 机制，发现需要实验引擎时转 research-workspace)
```

### 规则

| 条件 | Owner | 原因 |
|------|-------|------|
| 用户明确提及 templates/目录或实验参数配置 | research-workspace | 需要 research-harness 引擎执行 |
| 用户有针对现有代码的分解要求 | smoke | 更多是 skill 级分析工作流 |
| smoke 过程中需要运行可重复实验模板 | handoff to research-workspace | research_smoke MCP 工具 |
| 不确定时 | smoke（可 handoff） | smoke 的 workflow 更通用，实验模板场景可转 |

---

## 3. smoke ↔ systematic-debugging (L2, L0)

### 边界

| smoke | systematic-debugging |
|-------|---------------------|
| 已知功能/部件，测试其贡献 | 未知根因，追踪故障源头 |
| 广度优先 — 遍历所有部件 | 深度优先 — 一条路追到底 |
| "每个组件测一遍" | "为什么会挂/挂在哪" |
| 不要求复现故障 | 要求稳定复现故障 |

### 规则

| 条件 | Owner |
|------|-------|
| 用户有功能完整的系统，想测每块贡献 | smoke |
| 系统出问题了，不知道原因 | systematic-debugging |
| smoke 过程中发现某个部件表现异常 → 无法解释 | handoff to systematic-debugging |

---

## 4. smoke ↔ code-review-deep (L2, L2)

| smoke | code-review-deep |
|-------|-----------------|
| 测行为/功能/结果 | 审代码/逻辑/安全 |
| 可执行验证 | 静态分析 |
| "跑起来看结果" | "看代码找问题" |

### 规则

| 条件 | Owner |
|------|-------|
| "测试这个实现的功能完整性" | smoke |
| "审查这段代码的质量" | code-review-deep |
| 可以并行：先 smoke 验证功能，再 code-review-deep 审查代码 | 两个 skill 不互斥，顺序执行 |

---

## 5. 重路由指南

| 错误路由到 | 应当去 | 判别信号 |
|------------|--------|----------|
| deepinterview → 实际需要 smoke | 用户已有具体对象 | 用户回复「我有代码/方案，直接测」 |
| smoke → 实际需要 deepinterview | 用户没有具体对象 | 用户回复「还没确定实现思路，需要先想想」 |
| smoke → 实际需要 research-workspace | 需要实验模板执行引擎 | 「这个参数跑一下模板」 |
| research-workspace → 实际需要 smoke | 用户要做部件 ablation | 「每个模块去掉看看」 |
| smoke → 实际需要 systematic-debugging | 根因未知的故障 | smoke 发现不可解释的异常行为 |

### 跨 skill 产物流

| 产物 | 产出方 | 消费方 | 消费方式 |
|------|--------|--------|----------|
| 部件影响矩阵 | smoke | 研究者 | 决策是否裁剪/替换/优化组件 |
| 方案评估表 | smoke | 技术选型决策者 | 对比候选方案的收益和成本 |
| 异常发现 | smoke → systematic-debugging | systematic-debugging | smoke 发现某部件异常 → 发起根因调查 |
