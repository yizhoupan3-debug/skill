---
name: adversarial-loop
description: |
  Adversarial multi-pass review-fix-verify with progressive rubric disclosure (supervisor-led).
  Use when the user says `$adversarial-loop`, wants multi-pass adversarial criticism and fixes without
  revealing total iteration budget to the model, or references LOOP_ROUND_COMPLETE / progressive review tiers.
  Long-running round ledgers use the implicit Rust runtime `framework_rfv_loop` → `RFV_LOOP_STATE.json` (not a hot-routed skill).
  Do not use for Cursor interval/cron `/loop` wake tasks (see Cursor skills-cursor loop owner).
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
user-invocable: false
disable-model-invocation: true
risk: low
source: local
trigger_hints:
  - $adversarial-loop
  - adversarial loop
  - progressive disclosure review
  - LOOP_ROUND_COMPLETE
  - 对抗审查循环
  - 渐进披露审查
metadata:
  version: "1.0.2"
  platforms: [supported]
  tags: [adversarial-loop, adversarial, review, progressive-disclosure, hooks]
---

# adversarial-loop（对抗审查渐进披露循环）

显式入口：`$adversarial-loop`（**监督者**发起；模型默认不可自触发本 skill）。**不要**与 `loop` skill 的 interval `/loop` 唤醒混淆。

**宿主 hook 状态（重要）**：历史上的 `adversarial-loop-<session>.json` 注入路径已从 `router-rs` 移除（仅保留 SessionEnd 清扫与路径常量）。下面的 **轮次标记与 lane 契约仍可作为人工/监督协议**；长轮次、跨会话请用 **`framework_rfv_loop`** 写 `RFV_LOOP_STATE.json`（见 harness 参考 `rfv_loop_harness.md@{$FRAMEWORK_DOCS_GIT_REF}`，**不在热 skill 路由**）。

## 何时使用

- 需要多轮 **强对抗审查 → 针对性修复 → 可验证**，且希望 **按轮追加审查维度**（渐进披露）
- 已与用户约定使用 **`LOOP_ROUND_COMPLETE`** 标记一轮闭环
- 输出质量要求高（如安全审计、架构决策、关键 bug 修复），单轮 review 不足以收敛
- 需要跨子 agent 协作的审查流水线（reviewer / fixer / verifier 分离）

## 何时不要用

- interval/cron 式 `/loop 5m check status`（由 `loop` skill 负责）
- 单轮即可收敛、无验证命令
- 用户明确要求不要用 subagent / 不要多轮
- 纯粹的代码格式化或简单重构——这些不需要对抗审查

## 用户命令（协议）

| 命令 | 行为 |
|------|------|
| `$adversarial-loop <goal>` | 初始化对抗多轮目标（监督者显式） |
| `$adversarial-loop next` | 手动推进已完成轮次 |

## 核心执行流程

对抗循环按"审查 → 修复 → 验证"三阶段运转，每轮逐步提升审查深度。

### 阶段 1：Review（对抗审查）

每轮审查由独立 reviewer 子 agent 执行，职责：

1. **读取当前代码/产物**——基于上一轮 fix 的 diff，而非全量扫描
2. **按当前轮次的审查维度审查**——渐进披露意味着每轮只暴露一个新维度
3. **输出结构化发现**——每个 finding 包含：位置、问题描述、严重程度、建议修复方向

审查维度的渐进披露顺序（由监督者控制，不向 reviewer 暴露总轮数）：

| 轮次 | 审查维度 | 说明 |
|------|---------|------|
| R1 | 正确性 | 逻辑错误、边界条件、类型错误 |
| R2 | 健壮性 | 错误处理、异常路径、防御性编程 |
| R3 | 安全性 | 输入验证、权限检查、数据泄露风险 |
| R4 | 性能 | 热路径效率、内存使用、不必要的计算 |
| R5+ | 可维护性 | 命名、抽象、重复代码、文档覆盖 |

监督者可根据目标灵活调整顺序和维度，但**每轮只暴露一个维度**以维持对抗压力。

### 阶段 2：Fix（针对性修复）

fixer 执行修复时遵循：

1. **最小变更原则**——只修 reviewer 指出的问题，不做顺手重构
2. **保持测试通过**——修复不能破坏已有测试
3. **记录变更原因**——每处修改附带简短注释说明修复什么

修复完成后输出变更摘要（changed files + 修复点），供 verifier 使用。

### 阶段 3：Verify（验证）

verifier 独立于 reviewer 和 fixer，职责：

1. **运行验证命令**——编译、测试、lint 等自动化检查
2. **确认修复有效性**——reviewer 的 finding 是否被正确解决
3. **回归检查**——修复是否引入了新问题
4. **输出验证结论**——通过 / 未通过 + 具体原因

验证通过后，监督者标记 `LOOP_ROUND_COMPLETE`。

## 与 agent-swarm-orchestration 的衔接

对抗循环的 reviewer / fixer / verifier 三角色可映射到 agent swarm 的子 agent 模式：

- **Reviewer 子 agent**：spawn 时携带当前轮次的审查维度和代码上下文，不暴露总轮数
- **Fixer 子 agent**：spawn 时携带 reviewer 的 findings 和相关文件路径
- **Verifier 子 agent**：spawn 时携带 fixer 的变更摘要和验证命令

子 agent 之间通过文件产物传递状态（而非共享对话历史），确保各角色独立性。协调逻辑由监督者主 agent 管理。

详见 `skills/agent-swarm-orchestration/SKILL.md` 中的子 agent 编排模式。

## 轮次管理

### 最大轮次

默认最大 5 轮。超过 5 轮仍未收敛时：
1. 监督者评估是否为系统性问题（而非逐轮发现新问题）
2. 若是系统性问题，暂停循环，向用户报告根因分析
3. 用户可手动指定 `$adversarial-loop next` 强制继续

### 退出条件

满足以下**任一**条件即退出循环：

- **验证通过**：当前轮次 verifier 输出"通过"，且无新增 findings
- **达到最大轮次**：已达监督者设定的轮次上限
- **用户中断**：用户明确要求停止
- **收益递减**：连续两轮只发现 P3（低优先级）或 style 级别的问题

### 渐进披露的预算控制

监督者持有总轮数信息，**不向 reviewer / fixer / verifier 子 agent 暴露**。这是对抗性的核心设计——每个子 agent 都假设这是"最后一轮"，从而保持最大审查强度。

## 模型侧契约

每一轮建议：**独立 reviewer subagent → fixer → verifier**；深度与证据链见 `references/rfv-loop/reasoning-depth-contract.md@{$FRAMEWORK_DOCS_GIT_REF}` 与 lane 模板 `references/rfv-loop/lane-templates.md@{$FRAMEWORK_DOCS_GIT_REF}`。数理题另见 `references/rfv-loop/math-reasoning-harness.md@{$FRAMEWORK_DOCS_GIT_REF}`。

一轮结束时，助手可输出**独占一行**标记（区分大小写，行内不得有其他字符）：

```text
LOOP_ROUND_COMPLETE
```

## RFV 账本集成

长轮次或跨会话场景下，使用框架运行时的 RFV 循环管理：

- **启动**：`rfv_loop_manage operation=start goal="<审查目标>"`
- **追加轮次**：`rfv_loop_manage operation=append_round round=N review_summary="..." fix_summary="..."`
- **查看状态**：`rfv_loop_status`
- **产物位置**：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`

账本持久化了所有轮次的审查发现、修复摘要和验证结果，支持会话中断后的恢复。

## Hook 注入内容（历史摘要）

历史行为：在预算内于用户提交时追加 `additional_context`（tier + 对抗强度要求），**不披露**总轮数。当前树默认 **无** 该注入；需要同等能力时请用 **RFV 账本**（`artifacts/current/<task_id>/RFV_LOOP_STATE.json`）+ **`framework_rfv_loop`** stdio 或聊天续跑（**无** `RFV_LOOP_CONTINUE` hook 注入，2026-05 已拔除）。

## 常见使用场景

### 场景 1：安全敏感代码审查

```text
$adversarial-loop 审查 auth 模块的安全性
→ R1: 正确性（逻辑漏洞）
→ R2: 安全性（输入验证、token 泄露）
→ R3: 健壮性（错误处理、边界条件）
→ 收敛：3 轮后所有 finding 修复并通过验证
```

### 场景 2：架构重构后验证

```text
$adversarial-loop 验证状态机重构的正确性
→ R1: 正确性（状态转换是否完整）
→ R2: 健壮性（异常状态恢复）
→ R3: 可维护性（命名、文档）
→ 收敛：3 轮
```

### 场景 3：跨模块集成

```text
$adversarial-loop 验证新接口与现有模块的集成
→ R1: 正确性（接口契约、类型匹配）
→ R2: 性能（热路径效率）
→ R3: 安全性（权限检查）
→ R4: 可维护性（API 设计一致性）
→ 收敛：4 轮
```

## 验证

- 轮次账本与 stdio：`framework_rfv_loop`（`cd core/router-rs && cargo test` 覆盖 RFV）
- 路由元数据中的 skill 注册：`skills/SKILL_ROUTING_RUNTIME.json`
- Lane 模板与推理深度契约：`docs/references/rfv-loop/lane-templates.md`
