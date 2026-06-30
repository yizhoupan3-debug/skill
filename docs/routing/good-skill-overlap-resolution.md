# Routing Overlap Resolution

本文件记录 good-story 和 good-question 与框架既有 skill 的重叠边界与路由规则。

---

## 1. good-story ↔ paper-workbench (L2, L2)

### 重叠区域

两者都涉及论文的叙事/故事/主线。用户说「故事太弱」「主线不清晰」时可能同时匹配两者。

### 决策树

```
用户说"故事太弱" / "主线不清晰"
  ├── 用户有完整手稿 → paper-workbench
  │     (已经成稿，paper-workbench 的 hostile-but-fair 审稿覆盖故事诊断)
  │
  ├── 用户有零散结果/图表/草稿，尚未成稿 → good-story
  │     (故事诊断 + evidence map + 图序建议)
  │     ↓
  │     good-story 输出 Story Card → 用户据此撰写 → paper-workbench 审稿
  │
  └── 用户问"怎么讲故事" / 纯方法论问题 → good-story
        (不讲具体论文，只讲叙事的框架和原则)
```

### 规则

| 条件 | Owner | 原因 |
|------|-------|------|
| 有手稿草稿（哪怕粗糙） | paper-workbench | paper-workbench 的 loop mode 包含了 claim-evidence 检查 |
| 有结果/图表但无草稿 | good-story | good-story 专注于从材料中提取主线 |
| 纯方法论问题（"科学论文怎么讲故事"） | good-story | good-story 有 story-principles 参考卡 |
| 已有 Story Card，需要审稿/改写 | paper-workbench | good-story 的输出可以喂入 paper-workbench |

### 链式使用

```
good-story（故事诊断 → Story Card）→ paper-workbench（审稿 → 改写 → 投稿）
```

两阶段可顺序执行：`$good-story` 产出 Story Card 后，用户以此为指导写稿，完成后进入 `$paper-workbench`。两个 skill 不抢占同一次会话。

---

## 2. good-question ↔ research/discovery

### 重叠区域

research（discovery lane）描述中包含 "research-question scoping"（研究问题范围界定），而 good-question 的核心就是问题打磨。用户说"研究方向模糊""选题"时可能同时匹配。

### 决策树

```
用户说"研究方向模糊" / "选题"
  ├── 用户有具体文献/领域/问题可以调研 → research (discovery lane)
  │     (discovery lane 有 literature survey lane)
  │     ↓ (调研后问题仍需打磨)
  │     → good-question 进一步收敛
  │
  ├── 用户只有模糊兴趣/方向/idea → good-question
  │     (先打磨出好问题卡，再调研)
  │     ↓
  │     good-question 输出 Good Question Card → research (discovery lane) 按卡调研
  │
  └── 用户问纯方法论（"好问题怎么提出"） → good-question
```

### 规则

| 条件 | Owner | 原因 |
|------|-------|------|
| 用户有可搜索的关键词/问题/领域名 | research (discovery lane) | discovery lane 的调查能力更强（multi-source retrieval） |
| 用户只有模糊兴趣/困惑 | good-question | good-question 的 5 步管线（发散→收敛→scoring→卡）专为此场景设计 |
| 纯方法论（"如何提出好科研问题"） | good-question | good-question 有 Hamming/Fischbach/Alon/Peters 等参考文献 |
| 已有问题卡，需要验证或做文献定位 | research (discovery lane) | 狭域调研是 discovery lane 的核心能力 |

### 链式使用

```
good-question（问题打磨 → Good Question Card）
  → research（discovery lane — 带着问题做文献调研）
  → research（execution lane — 实验/数学/代码验证）
  → research（paper-workbench lane — 写稿/审稿/投稿）
```

---

## 3. good-story ↔ good-question (两者之间)

两者设计上互补而非重叠：

| 维度 | good-question | good-story |
|------|---------------|------------|
| 管线阶段 | 最前——选题 | 中后——叙事组织 |
| 输入 | 模糊兴趣/gap/idea | 结果/图表/草稿 |
| 输出 | Good Question Card | Story Card |
| 场景 | "这个问题值得做吗" | "这些结果怎么讲" |

### 链式组合（完整科研管线）

```
good-question（问题打磨）
  → research（discovery lane — 文献调研）
  → research（execution lane — 实验/代码/数学）
  → ... 产生结果后 ...
  → good-story（故事诊断）
  → research（paper-workbench lane — 审稿/改写/投稿）
```

---

## 4. 重路由指南

当 agent 错误路由时，按以下规则重路由：

| 错误路由到 | 应当去 | 判别信号 |
|------------|--------|----------|
| paper-workbench → 实际需要 good-story | 用户有零散结果而非完整稿子 | "我有实验结果但不知道怎么写" |
| good-story → 实际需要 paper-workbench | 用户有完整草稿 | "帮我审这篇稿子" |
| research（discovery lane）→ 实际需要 good-question | 用户只有模糊想法而非可调研的问题 | "我不知道该做什么方向" |
| good-question → 实际需要 research（discovery lane） | 已有问题明确 | "帮我查一下这部分的最新进展" |

### 跨 skill 产物流

两个外部 skill 的输出产物可以被框架其他 skill 消费：

| 产物 | 产出方 | 消费方 | 消费方式 |
|------|--------|--------|----------|
| Good Question Card | good-question | research（discovery lane） | 明确调研焦点 |
| Story Card | good-story | paper-workbench | 提供故事诊断 + weak points |
| Evidence Map | good-story | paper-workbench | 审稿的 claim 验证输入 |

---

## 5. QG Checker 集成评估

**结论：当前阶段不需要新增 QG checker。**

### 评估依据

good-story 和 good-question 是**交互式对话 skill**，不经过 GoalEngine 的 `task_complete` 退出门流程。现有 QG checker 注册表（`core/quality-gate/src/`）服务于自动管道任务（代码审查、论文验证等），其 scene=research 的 checker 链：

```
[LogicAndEvidence, Novelty, Math, Literature, ProseQC, Statistical, Reproducibility, Structure]
```

### 已知的协议合规缺口

深度路由审计发现以下框架协议缺口，标记为已知但不阻塞的架构债务：

| # | 缺口 | Severity | 说明 |
|---|------|----------|------|
| 1 | **缺少显式验证步骤** | P0 | 两个 skill 的工作流在产出卡片后都结束了，没有"验证→关闭"步骤，不符合框架 Runtime Protocol 的 `Task Intake → Execute → Verify` 闭环要求 |
| 2 | **缺少退出条件** | P1 | 两个 skill 都没有定义 stop rules / exit criteria（用户确认 / budget 耗尽 / 连续无 delta / 用户停止 / 仅剩 info） |
| 3 | **Checker 链过宽** | P1 | scene:research 的 8 个 checker 中，Math、Statistical 对 good-story 的叙事输出是噪声；Math、ProseQC、Reproducibility 对 good-question 的结构化卡片是噪声 |
| 4 | **无渐进披露** | P2 | 不像 paper-workbench 的 L0-L3 分层，两个 skill 都是线性展示全部工作流，用户看到的不一定都是他们需要的 |
| 5 | **无 delta-only 契约** | P2 | 输出格式可能重复用户上下文，不符合框架只携带 delta 的原则 |
| 6 | **零 research-harness MCP 工具使用** | P2 | paper-workbench 使用了 aigc_check、review_dimensions、claim_drift 等工具，但两个 skill 没有使用任何 Rust 层的研究工具 |

**修复计划**：
- 第 1-5 项：需要修改 SKILL.md 逻辑体本身。这会影响外部 skill 的便携性，建议在下次上游同步时评估是否将框架协议集成进核心工作流。
- 第 6 项：可以在不修改逻辑的情况下增加提示（"可调用 research-harness MCP tools"）。优先级低。

已经覆盖了这两个 skill 输出的质量维度（如果它们的产出被喂入 research（paper-workbench / execution lane）的自动管道）。

### 未来可能场景

如果未来出现以下情况，可以考虑添加 QG checker：

| 场景 | 需要的 checker | 触发条件 |
|------|---------------|----------|
| Story Card 需要被自动化管道消费 | `StoryCardWellFormed` | Story Card 有标准化 JSON schema |
| Good Question Card 需要被下游自动化 | `QuestionCardGate` | 卡片需要结构验证后才进 research（discovery lane） |

目前两个 skill 的输出都是人类可读的 markdown，不满足自动化消费的 schema 需求。等到管道化需求明确时再实现不迟。
