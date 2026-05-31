export const meta = {
  name: 'deep-review-template',
  description: '通用多 agent 深度审查模板：Scan → Merge → Verify → Synthesize 四阶段',
  phases: [
    { title: 'Scan', detail: '多维度并行扫描' },
    { title: 'Merge', detail: '保守去重合并' },
    { title: 'Verify', detail: '对抗性验证' },
    { title: 'Synthesize', detail: '结构化报告' },
  ],
}

// ── Schemas ──────────────────────────────────────────────────────────────────

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          severity: { type: 'string', enum: ['P0', 'P1', 'P2'] },
          title: { type: 'string' },
          file: { type: 'string' },
          line: { type: 'string' },
          description: { type: 'string' },
          evidence: { type: 'string' },
          fix_suggestion: { type: 'string' },
          lens: { type: 'string' },
        },
        required: ['severity', 'title', 'file', 'description', 'evidence', 'lens'],
      },
    },
  },
  required: ['findings'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    is_real: { type: 'boolean' },
    confirmed_severity: { type: 'string', enum: ['P0', 'P1', 'P2'] },
    root_cause: { type: 'string' },
    reasoning: { type: 'string' },
    fix_suggestion: { type: 'string' },
  },
  required: ['is_real', 'reasoning'],
}

// ── Dedup ────────────────────────────────────────────────────────────────────

function normalizeFile(f) { return (f.file || '').replace(/^\.\//, '').trim() }

function normalizeLine(line) {
  if (!line) return [0, 0]
  const parts = String(line).match(/\d+/g)
  if (!parts) return [0, 0]
  return [parseInt(parts[0]) || 0, parseInt(parts[parts.length - 1]) || parseInt(parts[0]) || 0]
}

function lineOverlap(a, b) {
  const [aS, aE] = normalizeLine(a), [bS, bE] = normalizeLine(b)
  if (aS === 0 || bS === 0) return 0
  const ov = Math.max(0, Math.min(aE, bE) - Math.max(aS, bS) + 1)
  return ov / Math.max(Math.min(aE - aS + 1, bE - bS + 1), 1)
}

function conservativeMerge(findings) {
  const groups = []
  for (const f of findings) {
    const nf = normalizeFile(f)
    let merged = false
    for (const g of groups) {
      if (normalizeFile(g[0]) === nf && g[0].lens === f.lens && lineOverlap(g[0].line, f.line) > 0.5) {
        g.push(f); merged = true; break
      }
    }
    if (!merged) groups.push([f])
  }
  return groups.map(g => {
    if (g.length === 1) return g[0]
    const order = { P0: 0, P1: 1, P2: 2 }
    g.sort((a, b) => (order[a.severity] ?? 99) - (order[b.severity] ?? 99))
    const best = { ...g[0] }
    const extra = g.slice(1).filter(f => f.evidence !== best.evidence).map(f => f.evidence)
    if (extra.length) best.evidence += '\n\n--- 补充 ---\n' + extra.join('\n')
    best.description += `（${g.length} 个扫描器报告）`
    return best
  })
}

// ── Lenses（按需自定义）─────────────────────────────────────────────────────
// 每个 lens 定义：label、prompt 模板、目标文件列表
// 复制本模板后，在此区域自定义扫描维度

const LENSES = [
  {
    label: 'correctness',
    lens: 'correctness',
    files: 'file1.rs, file2.rs',
    prompt: `审查逻辑 bug、边界条件、错误处理缺陷。维度：if/else 覆盖、unwrap 安全、状态一致性、边界输入。`,
  },
  {
    label: 'security',
    lens: 'security',
    files: 'file1.rs, file2.rs, script.sh',
    prompt: `审查安全问题。维度：SSRF、命令注入、路径遍历、权限提升、信息泄露、TOCTOU。`,
  },
  {
    label: 'consistency',
    lens: 'consistency',
    files: 'AGENTS.md, CLAUDE.md, docs/*.md',
    prompt: `审查跨文件一致性。维度：事实矛盾、路径引用、过时信息、语言混用、重复冗余。`,
  },
]

// ── Phase 1: Scan ────────────────────────────────────────────────────────────

phase('Scan')

const scanResults = await parallel(
  LENSES.map(cfg => () => agent(
    `面向用户的可见输出使用简体中文。

你是 ${cfg.label} 审查专家。

**目标文件**：${cfg.files}

**审查内容**：${cfg.prompt}

**质量要求**：
- 每个 finding 必须引用具体代码行（文件路径 + 行号范围）
- evidence 必须是实际代码片段，不是复述
- 仅报告你有充分证据的问题，不要猜测

请使用 Bash 工具的 cat 命令读取文件。
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体代码片段）、fix_suggestion、lens="${cfg.lens}"。`,
    { label: `scan:${cfg.label}`, phase: 'Scan', schema: FINDINGS_SCHEMA }
  ))
)

const failedAgents = scanResults.filter(r => !r).length
const allFindings = scanResults.filter(Boolean).flatMap(r => r.findings || [])
log(`Scan: ${allFindings.length} findings, ${failedAgents} agents failed`)

// ── Phase 2: Merge ───────────────────────────────────────────────────────────

phase('Merge')

const merged = conservativeMerge(allFindings)
log(`Merge: ${allFindings.length} → ${merged.length} (conservative, lens-aware)`)

// ── Phase 3: Verify ──────────────────────────────────────────────────────────

phase('Verify')

const verified = await pipeline(
  merged,
  (f, _orig, i) => agent(
    `面向用户的可见输出使用简体中文。

你是对立验证员。反驳以下 finding，只有证据充分且无法反驳时才确认。

**Finding #${i + 1}**: ${JSON.stringify({ severity: f.severity, title: f.title, file: f.file, line: f.line, lens: f.lens })}

**描述**: ${f.description}
**证据**: ${f.evidence}
**修复建议**: ${f.fix_suggestion || '未提供'}

**验证步骤**：
1. 读取源代码，确认 evidence 存在且引用正确
2. 分析上下文和设计意图
3. 判断：真实问题 or 有意设计？
4. 确认则给出根因分析 + 修复建议；误报则说明理由

请使用 Bash 工具的 cat 命令读取文件。
返回 JSON：is_real(boolean)、confirmed_severity、root_cause、reasoning、fix_suggestion。`,
    { label: `verify:${i}`, phase: 'Verify', schema: VERDICT_SCHEMA }
  )
).catch(() => merged.map(() => null))

// ── Phase 4: Synthesize ──────────────────────────────────────────────────────

phase('Synthesize')

const confirmed = [], rejected = []
merged.forEach((f, i) => {
  const v = verified[i]
  if (!v || !v.is_real) { rejected.push({ ...f, reason: v?.reasoning || 'no response' }); return }
  confirmed.push({ ...f, severity: v.confirmed_severity || f.severity, root_cause: v.root_cause, fix_suggestion: v.fix_suggestion || f.fix_suggestion, verdict: v.reasoning })
})
confirmed.sort((a, b) => ({ P0: 0, P1: 1, P2: 2 }[a.severity] ?? 99) - ({ P0: 0, P1: 1, P2: 2 }[b.severity] ?? 99))

log(`Done: ${confirmed.length} confirmed / ${rejected.length} rejected / ${failedAgents} agents failed`)

return {
  total_scanned: allFindings.length,
  confirmed_count: confirmed.length,
  findings: confirmed,
  rejected_count: rejected.length,
  coverage: {
    lenses_run: LENSES.map(l => l.lens),
    lenses_with_confirmed: [...new Set(confirmed.map(f => f.lens))],
    agents_failed: failedAgents,
  },
}
