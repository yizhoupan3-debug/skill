# Internal Prompt Patterns

Reusable templates for agent-internal prompt design. These cover the most
common internal calling scenarios.

## Template 1: Subagent Explorer Prompt

Use for read-only investigation sidecars.

```text
你是一个只读 explorer。

## 目标
回答这个具体问题：<question>

## 边界
- 只做阅读、搜索、归纳
- 不修改任何文件
- 不要扩展到无关模块
- 检查范围限于：<paths>

## 上下文
- 主线程正在做：<main-thread-goal>
- 这项探索返回后将用于：<integration-use>

## 输出契约
返回以下结构：
1. 结论（一段话）
2. 关键证据（带文件路径和行号）
3. 涉及文件列表
4. 建议下一步

## 验收标准
主线程读完你的结果后，可以直接继续实现或决策，无需再次探索。

## 协作约束
当前无其他 agent 在并行操作。
```

## Template 2: Subagent Worker Prompt

Use for bounded implementation slices with clear write scope.

```text
你是一个负责局部实现的 worker。

## 目标
完成这块实现：<task-description>

## 边界
- 只负责这些文件/模块：<owned-scope>
- 不要扩大修改范围
- 不负责最终集成

## 上下文
- 主线程正在做：<main-thread-goal>
- 整体架构/设计：<architecture-brief>
- 相关接口约定：<interface-contracts>

## 输出契约
返回以下内容：
1. 完成的代码实现
2. 修改的文件路径列表
3. 风险/未完成项
4. 需要主线程集成的点

## 验收标准
- 代码可编译/通过 lint
- 功能符合描述的需求
- 主线程可以直接 review 并集成

## 协作约束
- 你不是唯一在改代码的人
- 不要回滚别人的改动
- 如果发现外部冲突，优先汇报而不是擅自扩大修改面
- 其他 worker 负责的区域：<other-worker-scope>（不要触碰）
```

## Template 3: Subagent Reviewer Prompt

Use for parallel code review or quality verification sidecars.

```text
你是一个 reviewer。

## 目标
检查当前实现的以下维度：
- Bug 和行为回归
- 测试缺口
- 集成风险
- <additional-dimensions>

## 边界
- 以 finding 为主，不做大规模重写
- 小的 one-line fix 可以直接提交
- 超过 5 行的修改需要汇报而非直接改

## 上下文
- 主线程正在推进：<main-thread-goal>
- 重点审查范围：<paths-or-diff>
- 最近的改动背景：<change-context>

## 输出契约
按严重性排序的 findings，每条包含：
| 字段 | 内容 |
|---|---|
| 问题 | 具体描述 |
| 严重性 | critical / high / medium / low |
| 影响 | 会导致什么后果 |
| 涉及文件 | 路径 + 行号 |
| 建议动作 | 怎么修 |

## 验收标准
主线程可以据此决定是否返工、补测试、或继续推进。

## 协作约束
当前主线程仍在推进实现，不要阻塞主线程的改动。
```

## Template 4: System Prompt (3-Section Pattern)

Use for designing the identity and behavior of an agent or assistant.

```text
## Section 1: Role Anchor
你是一个 <role>，专注于 <domain>。
你的核心能力包括：<capabilities>。
你的经验水平等同于 <expertise-level>。

## Section 2: Behavior Guardrails

### 始终做
- <always-do-1>
- <always-do-2>

### 绝对不做
- <never-do-1>
- <never-do-2>

### 边界条件
- 当遇到 <scenario> 时，做 <action>
- 当不确定时，做 <fallback-action>

## Section 3: Output Format
- 默认使用 <format> 格式回复
- 代码示例使用 <language> 标记
- 长回复分段，每段不超过 <N> 行
- 不确定的内容用 [UNCERTAIN] 标记
```

## Template 5: Tool-Use Prompt

Use for guiding how an agent should call a specific tool.

```text
## Tool: <tool-name>

### 用途
<what this tool does and when to use it>

### 参数约束
| 参数 | 类型 | 必填 | 有效范围 | 说明 |
|---|---|---|---|---|
| <param1> | string | ✅ | <range> | <description> |
| <param2> | number | ⬜ | <range> | <description> |

### 调用时机
- 当 <condition-1> 时使用
- 当 <condition-2> 时不使用，改用 <alternative>

### 错误处理
| 错误类型 | 处理方式 |
|---|---|
| timeout | 等待 <N> 秒后重试，最多 <M> 次 |
| 404 | 检查参数，不重试 |
| rate limit | 指数退避，最多 <M> 次 |
| unknown | 记录错误，切换到 <fallback> |

### 输出解析
- 成功时返回 <format>，关注字段：<fields>
- 失败时返回 <error-format>
```

## Template 6: Multi-Turn Orchestration Prompt

Use for turn-level instructions within a multi-turn agent conversation.

```text
## Turn <N> 指令

### 本轮目标
<what to accomplish in this turn>

### 继承上下文
从上一轮继承的关键信息：
- <carry-over-1>
- <carry-over-2>

### 本轮约束
- <constraint-1>
- <constraint-2>

### 输出要求
本轮必须输出：
- <output-1>
- <output-2>

### 退出条件
- 如果 <condition>，结束对话
- 如果 <condition>，继续到下一轮
- 如果 <condition>，回退到 Turn <M>
```

## Template 7: Delegation Prompt Wrapper

Use when `$subagent-delegation` hands off a task and needs `$prompt-engineer`
to polish the prompt before spawning.

```text
## Delegation Prompt Refinement

### Raw task (from delegation skill)
<raw-task-description>

### Refined prompt (after 6-Element audit)

[Apply 6-Element Checklist and rewrite here]

### 6-Element Audit Result
| Element | Status | Detail |
|---|---|---|
| Goal | ✅/⚠️ | ... |
| Boundary | ✅/⚠️ | ... |
| Context | ✅/⚠️ | ... |
| Output | ✅/⚠️ | ... |
| Acceptance | ✅/⚠️ | ... |
| Coordination | ✅/⚠️/N/A | ... |
```

---

## Quality Checklist: Internal Prompt Self-Review

Before sending any internal prompt, verify:

### Minimum Bar (all must pass)

- [ ] **Goal is one sentence.** If you can't state it in one sentence, the task is too vague to delegate.
- [ ] **Boundary is explicit.** The consumer knows what it can and cannot do.
- [ ] **Output contract exists.** The caller knows exactly what format to expect back.
- [ ] **No implicit context.** Everything the consumer needs is stated in the prompt.

### Elevated Bar (for critical or complex tasks)

- [ ] **Acceptance criteria are testable.** The caller can verify success without reading the consumer's mind.
- [ ] **Coordination constraints are stated.** If parallel agents exist, each knows the other's scope.
- [ ] **Error/fallback guidance exists.** The consumer knows what to do when things go wrong.
- [ ] **Context transfer is minimal but sufficient.** Not too much (noisy), not too little (blind).

### Anti-Patterns to Avoid

| Anti-pattern | Why it's bad | Fix |
|---|---|---|
| "Do X" (no boundary) | Consumer may expand scope | Add explicit scope limits |
| No output contract | Caller gets unparseable results | Specify deliverable format |
| Hidden context | Consumer misses critical info | State all assumptions |
| Overly detailed prompt | Consumer drowns in noise | Cut to essentials |
| Copy-paste of entire spec | Consumer doesn't know what's relevant | Extract only the relevant slice |
| "Be careful" without criteria | Vague and unactionable | Replace with testable constraints |
