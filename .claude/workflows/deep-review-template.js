import { agent, pipeline, phase, log } from 'workflow'
import {
  FINDINGS_SCHEMA, BATCH_VERDICT_SCHEMA, FACTCHECK_VERDICT_SCHEMA,
  normalizeFile, normalizeLine, lineOverlap, conservativeMerge,
} from './workflow-helpers.js'

export const meta = {
  name: 'deep-review-template',
  description: '通用多 agent 深度审查模板：Scan → Merge → Verify → Synthesize 四阶段',
  phases: [
    { title: 'Scan', detail: '多维度串行扫描' },
    { title: 'Merge', detail: '保守去重合并' },
    { title: 'Verify', detail: '对抗性验证' },
    { title: 'Synthesize', detail: '结构化报告' },
  ],
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

// 串行扫描：用 pipeline 逐个 lens 审查，不加并发
const scanResults = await pipeline(
  LENSES,
  (cfg, orig, i) => agent(
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
  )
).catch(() => LENSES.map(() => null))

const failedAgents = scanResults.filter(r => !r).length
const allFindings = scanResults.filter(Boolean).flatMap(r => r.findings || [])
log(`Scan: ${allFindings.length} findings, ${failedAgents} agents failed`)

// ── Phase 2: Merge ───────────────────────────────────────────────────────────

phase('Merge')

const merged = conservativeMerge(allFindings)
log(`Merge: ${allFindings.length} → ${merged.length} (conservative, lens-aware)`)

// ── Phase 3: Verify ──────────────────────────────────────────────────────────

phase('Verify')

// 批量验证：所有 findings 打包给一个 agent，避免 N 个 finding → N 个 agent
// findings 较多时自动追加分段；替换逐条 pipeline 以减少 token 和 agent 数量
const findingsForVerify = merged.map((f, i) => ({
  index: i,
  severity: f.severity,
  title: f.title,
  file: f.file,
  line: f.line,
  lens: f.lens,
  description: f.description,
  evidence: f.evidence,
  fix_suggestion: f.fix_suggestion || '未提供',
}))

const verifyListing = findingsForVerify.map(f =>
  `[#${f.index}] ${f.severity} | ${f.file}:${f.line} | ${f.lens}
  标题: ${f.title}
  描述: ${f.description}
  证据: ${f.evidence}
  修复建议: ${f.fix_suggestion}`
).join('\n\n---\n\n')

const verifiedBatch = await agent(
  `面向用户的可见输出使用简体中文。

你是对立验证员。以下共 ${findingsForVerify.length} 个发现，请对每个独立判断是真问题还是误报。

逐一审核：

${verifyListing}

**验证步骤**：
1. 读取源代码，确认 evidence 存在且引用正确
2. 分析上下文和设计意图
3. 判断：真实问题 or 有意设计？
4. 真实问题给出根因 + 修复建议；误报说明理由

请使用 Bash 工具的 cat 命令读取文件验证证据。
返回 JSON: { verdicts: [{ finding_index, is_real, confirmed_severity, root_cause, reasoning, fix_suggestion }] }`,
  { label: 'verify:batch', phase: 'Verify', schema: BATCH_VERDICT_SCHEMA }
)

const verdictMap = {}
if (verifiedBatch?.verdicts) {
  for (const v of verifiedBatch.verdicts) {
    verdictMap[v.finding_index] = v
  }
}
const verified = merged.map((f, i) => verdictMap[i] || null)

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
