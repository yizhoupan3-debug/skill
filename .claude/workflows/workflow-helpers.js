/**
 * Workflow shared helpers — schemas, dedup, merge utilities.
 * Extracted from claude-chain-deep-review, claude-code-cli-audit, deep-review-template.
 */

// ── Schemas ──────────────────────────────────────────────────────────────────

export const FINDINGS_SCHEMA = {
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

export const AUDIT_FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          severity: { type: 'string' },
          category: { type: 'string' },
          title: { type: 'string' },
          description: { type: 'string' },
          location: { type: 'string' },
          impact: { type: 'string' },
          recommendation: { type: 'string' },
        },
        required: ['severity', 'category', 'title', 'description', 'location', 'recommendation']
      }
    },
    summary: { type: 'string' },
  },
  required: ['findings', 'summary']
}

export const VERDICT_SCHEMA = {
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

/**
 * 批量验证 schema：一个 agent 审查所有 findings，返回按 index 对应的裁决数组。
 * 替代 pipeline 逐条验证，减少 token 消耗和 agent 数量。
 */
export const BATCH_VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          finding_index: { type: 'integer' },
          is_real: { type: 'boolean' },
          confirmed_severity: { type: 'string', enum: ['P0', 'P1', 'P2'] },
          root_cause: { type: 'string' },
          reasoning: { type: 'string' },
          fix_suggestion: { type: 'string' },
        },
        required: ['finding_index', 'is_real', 'reasoning'],
      },
    },
  },
  required: ['verdicts'],
}

// ── Helpers ──────────────────────────────────────────────────────────────────

export const READ_INSTRUCTION = `请使用 Bash 工具的 cat 命令读取文件（不要用 Read 工具，因为框架保护了这些文件）。`

export function normalizeFile(f) {
  return (f.file || '').replace(/^\.\//, '').trim()
}

export function normalizeLine(line) {
  if (!line) return [0, 0]
  const parts = String(line).match(/\d+/g)
  if (!parts) return [0, 0]
  return [parseInt(parts[0]) || 0, parseInt(parts[parts.length - 1]) || parseInt(parts[0]) || 0]
}

export function lineOverlap(a, b) {
  const [aStart, aEnd] = normalizeLine(a)
  const [bStart, bEnd] = normalizeLine(b)
  if (aStart === 0 || bStart === 0) return 0
  const overlap = Math.max(0, Math.min(aEnd, bEnd) - Math.max(aStart, bStart) + 1)
  const aLen = Math.max(aEnd - aStart + 1, 1)
  const bLen = Math.max(bEnd - bStart + 1, 1)
  return overlap / Math.min(aLen, bLen)
}

/**
 * 保守去重：仅在 (file + lens + 行号重叠>50%) 时合并。
 * 不同 lens 保留为独立条目（同一行代码的安全问题和正确性问题是不同发现）。
 */
export function conservativeMerge(findings) {
  const groups = []
  for (const f of findings) {
    const nf = normalizeFile(f)
    let merged = false
    for (const g of groups) {
      const gg = g[0]
      if (
        normalizeFile(gg) === nf &&
        gg.lens === f.lens &&
        lineOverlap(gg.line, f.line) > 0.5
      ) {
        g.push(f)
        merged = true
        break
      }
    }
    if (!merged) groups.push([f])
  }
  return groups.map(g => {
    if (g.length === 1) return g[0]
    // 合并：保留最高 severity，合并 evidence，保留所有来源
    const severityOrder = { P0: 0, P1: 1, P2: 2 }
    g.sort((a, b) => (severityOrder[a.severity] ?? 99) - (severityOrder[b.severity] ?? 99))
    const best = { ...g[0] }
    const extraEvidence = g.slice(1).filter(f => f.evidence !== best.evidence).map(f => f.evidence)
    if (extraEvidence.length > 0) {
      best.evidence = best.evidence + '\n\n--- 补充证据 ---\n' + extraEvidence.join('\n')
    }
    best.description = best.description + `（${g.length} 个独立扫描器报告此问题）`
    return best
  })
}

export const FACTCHECK_VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    is_accurate: { type: 'boolean' },
    errors: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          quote: { type: 'string' },
          correction: { type: 'string' },
          severity: { type: 'string', enum: ['minor', 'major', 'critical'] }
        },
        required: ['quote', 'correction', 'severity']
      }
    },
    reasoning: { type: 'string' }
  },
  required: ['is_accurate', 'errors', 'reasoning']
}
