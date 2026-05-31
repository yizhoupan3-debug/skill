# Workflow 脚本编写约定

> 适用于 `.claude/workflows/` 下的 Dynamic Workflow JS 脚本。
> 参考：Claude Code Best Practices、agent-swarm-orchestration 规范、reasoning-depth-contract。

## 阶段结构（最少 4 阶段）

| 阶段 | 职责 | 模式 |
|------|------|------|
| **Scan** | 多维度并行只读扫描 | `parallel([() => agent(...)])` |
| **Merge** | 保守去重合并 | 主线程纯代码（不调 agent） |
| **Verify** | 对抗性逐条验证 | `pipeline(items, ...)` |
| **Synthesize** | 排序、覆盖度分析、报告 | 主线程纯代码 |

单阶段（只有 Scan）或两阶段（Scan → Verify 跳过 Merge）均为不完整结构。

## Scan 阶段约定

1. 每个 agent 只负责一个审查维度（lens），prompt 中明确 `lens="xxx"`
2. 并行 agent 数 ≤ 16（运行时上限）
3. 所有 agent 使用同一个 `FINDINGS_SCHEMA`，schema 中 `required` 字段不可省略 `evidence`
4. Prompt 首行写 `面向用户的可见输出使用简体中文`
5. 每个 agent 的 prompt 末尾追加质量要求：「仅报告你有充分证据的问题，不要猜测」
6. agent 失败用 `scanResults.filter(Boolean)` 处理，记录 `failedAgents` 数量

## Merge 阶段约定

**保守去重原则**：宁可保留冗余，不可误删独立发现。

- 合并条件：`file` 相同 + `lens` 相同 + 行号重叠 > 50%
- 不同 `lens` 的 findings **不合并**（同一行的安全问题和正确性问题是不同发现）
- 合并时保留最高 severity，拼接 evidence，标注来源数量
- Merge 在主线程用纯代码完成（不调 agent，不消耗额外 token）

## Verify 阶段约定

- 使用 `pipeline(items, ...)` 逐条验证（非并行），每条 finding 独立验证 agent
- Prompt 以「反驳以下 finding」开头（对抗性而非确认性）
- 要求验证 agent 读取实际代码确认 evidence 存在
- `VERDICT_SCHEMA` 必须包含 `is_real`（boolean）和 `reasoning`（string）
- 对 `pipeline` 外层包 `.catch()`，防止单个 agent 异常导致整条链崩溃

## Synthesize 阶段约定

- 合并验证结果：confirmed / rejected 分别收集
- 按 severity 排序（P0 > P1 > P2）
- 计算覆盖度（哪些 lens 运行了、哪些有 confirmed、几个 agent 失败）
- 返回值包含 `coverage` 对象，让调用方知道哪些维度未覆盖

## Schema 设计

- Finding 核心字段：`severity`, `title`, `file`, `line`, `description`, `evidence`, `fix_suggestion`, `lens`
- Verdict 核心字段：`is_real`, `reasoning`；可选 `confirmed_severity`, `root_cause`, `fix_suggestion`
- `evidence` 必须是 `required`（不允许无证据断言）

## Budget 感知

当脚本由 Ultracode 或用户指定 token 目标时：
- 用 `budget.total` / `budget.spent()` / `budget.remaining()` 追踪
- Scan 阶段占 60%，Verify 阶段占 25%，余量 15%
- 循环中检查 `budget.remaining()` 防止超支

## 反模式（禁止）

| 反模式 | 为什么不行 |
|--------|-----------|
| 仅基于 title+file 精确去重 | 不同措辞的同一发现会被误删 |
| Scan 和 Verify 合并到同一 agent | 违反「执行者不自评」原则 |
| Verify 用 `parallel` 而非 `pipeline` | 验证需要统一上下文，且并行消耗过多 token |
| 无证据的 finding 通过验证 | schema 中 `evidence` 不是 required 的后果 |
| pipeline 内无 `.catch()` | 一个 agent 失败导致整条链中断 |
| 只有 Scan+Verify，无 Merge | 24 个原始 findings 可能被 Verify 过滤为 0 |

---

# Workflow 语法速查与常见错误修复

## 核心 API 语法对比

### `parallel(thunks)` — 并行执行多个任务
**用途**：多个 agent 同时工作（如 Scan 阶段的多维度审查）

```javascript
// ✅ 正确：传入 thunk 数组
const results = await parallel([
  () => agent("prompt1", { schema: SCHEMA }),
  () => agent("prompt2", { schema: SCHEMA }),
  () => agent("prompt3", { schema: SCHEMA })
])

// ❌ 错误：传多个参数
await parallel(
  agent("prompt1", {...}),  // TypeError: parallel requires array
  agent("prompt2", {...}),
  agent("prompt3", {...})
)
```

**要点**：
- 必须传入 **数组**（`[...]`），数组元素是 **thunk 函数**（`() => agent(...)`）
- 每个 thunk 返回 Promise
- 某个 agent 失败不影响其他（返回 `null`，需 `filter(Boolean)` 处理）

---

### `pipeline(items, ...stages)` — 串行多阶段处理
**用途**：item 经过多个处理阶段（如 Verify 阶段的逐条验证）

```javascript
// ✅ 正确：items 数组 + 各阶段回调
const verified = await pipeline(
  findings,                          // 要处理的 items
  (item, origItem, i) => agent(     // 第1阶段：验证
    `验证 ${item.title}`,
    { schema: VERDICT_SCHEMA }
  ),
  (verdict, origItem, i) => transform(verdict) // 第2阶段：转换
).catch(() => findings.map(() => null))

// ❌ 错误：items 是单个值
await pipeline(findings[0], ...)  // ❌ 不支持单个 item
```

**要点**：
- 第一个参数是 **items 数组**
- 后续参数是 **stage 回调**：`(prevResult, originalItem, index) => ...`
- Stage 之间通过 prevResult 传递数据
- 推荐在末尾加 `.catch()` 防止单个失败中断整链

---

### `agent(prompt, opts)` — 执行单个 agent
**用途**：独立完成一个子任务（如 Synthesize 阶段的报告生成）

```javascript
// ✅ 正确
const result = await agent(
  `分析 ${data.length} 条记录，生成 JSON 报告`,
  {
    label: 'analyzer',
    phase: 'Synthesize',
    schema: OUTPUT_SCHEMA
  }
)
```

---

### `phase(title)` — 标记阶段进度
**用途**：在 UI 中显示阶段名称

```javascript
phase('Scan')      // 显示进度
phase('Merge')
phase('Verify')
```

---

## 常见错误与修复速查表

### 错误 1: `TypeError: parallel requires array`
**症状**：Workflow 启动失败，报 `parallel requires array`
**原因**：parallel() 接收了多个参数，而不是数组

```javascript
// ❌ 错误
await parallel(agent("a", {}), agent("b", {}))

// ✅ 修复
await parallel([() => agent("a", {}), () => agent("b", {})])
```

---

### 错误 2: `TypeError: pipeline requires at least 2 arguments`
**症状**：pipeline 启动失败
**原因**：pipeline 缺少 items 数组或 stage 回调

```javascript
// ❌ 错误
await pipeline(agent("a", {}))

// ✅ 修复
await pipeline(
  items,
  (item, orig, i) => agent(`处理 ${item}`, {...})
)
```

---

### 错误 3: `schema validation failed`
**症状**：agent 返回 null，schema 验证失败
**原因**：返回的 JSON 缺少 required 字段

```javascript
// ❌ 错误：schema 中 required 包含 'evidence' 但返回时遗漏
const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        properties: {
          severity: { type: 'string' },
          evidence: { type: 'string' }  // ← required 但可能返回空
        },
        required: ['severity', 'title', 'evidence']  // ← evidence 必须
      }
    }
  }
}

// ✅ 修复：确保 prompt 要求提供 evidence
`...请为每个 finding 提供 evidence 字段（代码片段），不可省略...`
```

---

### 错误 4: `Unhandled rejection: one agent failure stops everything`
**症状**：一个 agent 失败导致整条链崩溃
**原因**：pipeline 或 parallel 末尾没有 `.catch()`

```javascript
// ❌ 错误：agent 失败会中断
const results = await pipeline(items, stage)

// ✅ 修复：用 .catch() 处理失败
const results = await pipeline(items, stage)
  .catch(() => items.map(() => null))
```

---

### 错误 5: `Duplicate findings after merge`
**症状**：合并后仍有重复 findings
**原因**：保守合并条件太松或没有按 lens 区分

```javascript
// ❌ 错误：仅按 title+file 去重
function isDuplicate(a, b) {
  return a.title === b.title && a.file === b.file
}

// ✅ 修复：按 file + lens + line 重叠度合并
function isDuplicate(a, b) {
  const f = a.file === b.file
  const l = a.lens === b.lens
  const overlap = lineOverlap(a.line, b.line) > 0.5
  return f && l && overlap
}
```

---

### 错误 6: `Budget exceeded`
**症状**：workflow 运行中途因 token 超支终止
**原因**：没有追踪 budget.remaining()

```javascript
// ❌ 错误：没有检查 budget
const findings = []
while (findings.length < 20) {
  const batch = await agent("扫描更多", {...})
  findings.push(...batch)
}

// ✅ 修复：每轮检查 budget
while (findings.length < 20) {
  if (budget.total && budget.remaining() < 50_000) {
    log(`Budget 剩余 ${budget.remaining()/1000}k，停止`)
    break
  }
  const batch = await agent("扫描更多", {...})
  findings.push(...batch)
}
```

---

## 最小可运行示例

### 1. 最简单 workflow（单阶段 Scan）

```javascript
export const meta = {
  name: 'my-minimal-workflow',
  description: '最小示例：单个扫描 agent',
  phases: [{ title: 'Scan' }]
}

const schema = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          severity: { type: 'string', enum: ['P0', 'P1', 'P2'] }
        },
        required: ['title', 'severity']
      }
    }
  },
  required: ['findings']
}

phase('Scan')

const result = await agent(
  '分析代码库，找出关键问题。返回 { findings: [{title, severity}] }',
  { label: 'scanner', phase: 'Scan', schema }
)

log(`发现 ${result?.findings?.length || 0} 个问题`)
return result
```

---

### 2. 完整四阶段 workflow

```javascript
export const meta = {
  name: 'my-full-workflow',
  description: '完整示例：Scan → Merge → Verify → Synthesize',
  phases: [
    { title: 'Scan', detail: '多维度扫描' },
    { title: 'Merge', detail: '去重合并' },
    { title: 'Verify', detail: '验证确认' },
    { title: 'Synthesize', detail: '生成报告' }
  ]
}

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          severity: { type: 'string', enum: ['P0', 'P1', 'P2'] },
          title: { type: 'string' },
          file: { type: 'string' },
          description: { type: 'string' },
          evidence: { type: 'string' },
          lens: { type: 'string' }
        },
        required: ['severity', 'title', 'file', 'description', 'evidence', 'lens']
      }
    }
  },
  required: ['findings']
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    is_real: { type: 'boolean' },
    reasoning: { type: 'string' }
  },
  required: ['is_real', 'reasoning']
}

// ── Scan ──
phase('Scan')

const LENSES = [
  { label: 'correctness', lens: 'correctness', prompt: '审查逻辑 bug' },
  { label: 'security', lens: 'security', prompt: '审查安全漏洞' }
]

const scanResults = await parallel(
  LENSES.map(cfg => () => agent(
    `面向用户的可见输出使用简体中文。
    ${cfg.prompt}。返回 JSON { findings: [...] }`,
    { label: `scan:${cfg.label}`, phase: 'Scan', schema: FINDINGS_SCHEMA }
  ))
)

const allFindings = scanResults.filter(Boolean).flatMap(r => r.findings || [])
log(`Scan: ${allFindings.length} findings`)

// ── Merge ──
phase('Merge')

function merge(findings) {
  const grouped = []
  for (const f of findings) {
    const existing = grouped.find(g =>
      g.file === f.file && g.lens === f.lens
    )
    if (existing) {
      existing.items.push(f)
    } else {
      grouped.push({ ...f, items: [f] })
    }
  }
  return grouped.map(g => ({
    ...g.items[0],
    description: g.items.length > 1
      ? `${g.items.length} 个报告: ${g.items.map(i => i.description).join('; ')}`
      : g.items[0].description
  }))
}

const merged = merge(allFindings)
log(`Merge: ${allFindings.length} → ${merged.length}`)

// ── Verify ──
phase('Verify')

const verified = await pipeline(
  merged,
  (f, orig, i) => agent(
    `反驳以下 finding，只有证据充分时才确认。
    Finding: ${JSON.stringify({ title: f.title, file: f.file })}
    Evidence: ${f.evidence}
    返回 JSON: { is_real: boolean, reasoning: string }`,
    { label: `verify:${i}`, phase: 'Verify', schema: VERDICT_SCHEMA }
  )
).catch(() => merged.map(() => null))

// ── Synthesize ──
phase('Synthesize')

const confirmed = merged.filter((f, i) => verified[i]?.is_real)
const rejected = merged.filter((f, i) => !verified[i]?.is_real)

log(`Synthesize: ${confirmed.length} confirmed, ${rejected.length} rejected`)

return {
  confirmed_count: confirmed.length,
  findings: confirmed,
  rejected_count: rejected.length
}
```

---

## 调试技巧

### 1. 查看 workflow 日志
```bash
/workflows
```
UI 会显示各阶段的进度和 agent 输出

### 2. 检查 schema 验证失败
在 agent prompt 中明确要求：
```
返回严格的 JSON 格式，确保 required 字段都存在。
如果某个字段不存在，请用合理默认值填充。
```

### 3. 处理并行失败
```javascript
const results = await parallel([...])
const successful = results.filter(Boolean)
const failed = results.length - successful.length
if (failed > 0) {
  log(`⚠️ ${failed} 个 agent 失败，继续处理 ${successful.length} 个结果`)
}
```

### 4. 调试单个 agent
先不传 schema，直接让 agent 返回文本：
```javascript
const debugResult = await agent("检查这段代码: ...")
log(`Debug output: ${JSON.stringify(debugResult)}`)
```
确认 agent 行为正确后，再加回 schema。

---

## 最佳实践清单

- [ ] parallel() 传入 thunk 数组，不是多个参数
- [ ] pipeline() 传入 items 数组 + stages 回调，不是单个值
- [ ] 每个 pipeline 末尾加 `.catch()` 处理失败
- [ ] schema 中 `required` 字段必须有合理默认值或 prompt 要求
- [ ] Scan 阶段用 parallel()，Verify 阶段用 pipeline()
- [ ] 用 `phase()` 标记阶段进度
- [ ] 用 `log()` 输出关键信息供调试
- [ ] 追踪 `budget.remaining()` 防止 token 超支
- [ ] 某个 agent 失败时用 `filter(Boolean)` 处理，不要中断整条链
- [ ] 所有 finding 必须有 `evidence` 字段（代码片段）
