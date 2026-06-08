import { agent, parallel, pipeline, phase, log } from 'workflow'
import {
  FINDINGS_SCHEMA, VERDICT_SCHEMA, FACTCHECK_VERDICT_SCHEMA, READ_INSTRUCTION,
  normalizeFile, normalizeLine, lineOverlap, conservativeMerge,
} from './workflow-helpers.js'

export const meta = {
  name: 'claude-chain-deep-review',
  description: '全面深度审查 Claude 全链路（hooks、host integration、install scripts、配置、文档），找出真实问题',
  phases: [
    { title: 'Scan', detail: '多维度并行扫描所有 Claude 链路文件' },
    { title: 'Merge', detail: '保守去重合并 findings，不同 lens 保留独立条目' },
    { title: 'Verify', detail: '对立验证每个 finding，确认为真实问题' },
    { title: 'Synthesize', detail: '按 severity 排序，生成最终结构化报告' },
  ],
}

const REPORT_SCHEMA = {
  type: 'object',
  properties: {
    total_scanned: { type: 'number' },
    confirmed_count: { type: 'number' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          severity: { type: 'string' },
          title: { type: 'string' },
          file: { type: 'string' },
          line: { type: 'string' },
          description: { type: 'string' },
          evidence: { type: 'string' },
          root_cause: { type: 'string' },
          fix_suggestion: { type: 'string' },
          lens: { type: 'string' },
          verdict: { type: 'string' },
        },
        required: ['severity', 'title', 'file', 'description', 'evidence', 'lens', 'verdict'],
      },
    },
    rejected_count: { type: 'number' },
    coverage: {
      type: 'object',
      properties: {
        lenses_run: { type: 'array', items: { type: 'string' } },
        lenses_with_confirmed: { type: 'array', items: { type: 'string' } },
        agents_failed: { type: 'number' },
      },
    },
  },
  required: ['total_scanned', 'confirmed_count', 'findings', 'coverage'],
}

// ── Phase 1: Scan ────────────────────────────────────────────────────────────

phase('Scan')

const scanResults = await parallel([
  () => agent(
    `面向用户的可见输出使用简体中文。

你是一个严格的代码正确性审查员。深度审查以下文件中的逻辑 bug、边界条件、错误处理缺陷：

**文件列表**：
- core/router-rs/src/hosts/claude_hooks.rs（Claude Code 四事件 hook）
- core/router-rs/src/hosts/claude_desktop_hooks.rs（Claude Desktop MCP hook）
- core/router-rs/src/host_integration/mod.rs（host 集成安装逻辑）

**审查维度**：
1. 逻辑错误：if/else 分支覆盖不全、match arm 缺失、条件判断反转
2. 错误处理：unwrap() 在可失败路径、错误被吞掉、fail-open vs fail-closed 不一致
3. 状态一致性：hook state 文件的读写竞争、JSON 解析容错
4. 边界条件：空输入、缺失文件、损坏 JSON、超大 payload
5. 重复逻辑：跨文件的重复实现是否一致

**质量要求**：
- 每个 finding 必须引用具体代码行（文件路径 + 行号范围）
- evidence 必须是实际代码片段，不是复述
- 仅报告你有充分证据的问题，不要猜测

${READ_INSTRUCTION}
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体代码片段）、fix_suggestion、lens="correctness"。`,
    { label: 'scan:correctness', phase: 'Scan', schema: FINDINGS_SCHEMA }
  ),

  () => agent(
    `面向用户的可见输出使用简体中文。

你是一个安全审查专家。深度审查 Claude 全链路的安全问题：

**文件列表**：
- core/router-rs/src/hosts/claude_hooks.rs
- core/router-rs/src/hosts/claude_desktop_hooks.rs
- core/router-rs/src/host_integration/mod.rs
- core/router-rs/src/web_fetch_guard.rs
- core/router-rs/src/hook_outbound_protect.rs
- core/router-rs/src/hook_policy.rs
- scripts/install-claude.sh
- scripts/install-claude-desktop.sh

**审查维度**：
1. SSRF：web_fetch 的 host 白名单是否完整、绕过方式
2. 命令注入：shell 脚本中的变量是否正确引用、stdin 拼接
3. 路径遍历：文件路径是否做了 sanitize
4. 权限提升：安装脚本的权限范围是否过宽
5. 信息泄露：错误信息是否暴露内部路径
6. 时序攻击：hook state 的 TOCTOU 问题

**质量要求**：
- 每个 finding 必须引用具体代码行（文件路径 + 行号范围）
- evidence 必须是实际代码片段，不是复述
- 仅报告你有充分证据的问题，不要猜测

${READ_INSTRUCTION}
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体代码片段）、fix_suggestion、lens="security"。`,
    { label: 'scan:security', phase: 'Scan', schema: FINDINGS_SCHEMA }
  ),

  () => agent(
    `面向用户的可见输出使用简体中文。

你是一个文档与配置一致性审查员。审查 Claude 全链路中跨文件的不一致问题：

**文件列表**：
- AGENTS.md（跨宿主内核）
- AGENTS_CLAUDE.md（Claude 宿主专用）
- .claude/CLAUDE.md（项目级 Claude 配置）
- .claude/rules/framework.md
- docs/hosts/claude-desktop.md
- docs/hosts/claude.md
- docs/harness_architecture.md
- docs/harness_policy_map.md
- docs/closeout_enforcement.md
- .claude/settings.json
- configs/framework/claude-router-rs-hook.sh
- configs/framework/claude-router-rs-hook.env

**审查维度**：
1. 事实不一致：同一概念在不同文件中的描述是否矛盾（如 lifecycle_profile、hook 数量、门控行为）
2. 路径引用：文档中引用的文件路径是否实际存在
3. 过时信息：描述是否与最近的代码改动匹配
4. 语言混用：中英文混排问题
5. 重复冗余：相同信息在多处重复且不一致
6. CLAUDE.md / settings.json 中的 hook 命令是否与 claude-router-rs-hook.sh 一致

**质量要求**：
- 每个 finding 必须引用两个文件中的具体矛盾内容
- evidence 必须是实际文件内容的引用
- 仅报告你有充分证据的问题，不要猜测

${READ_INSTRUCTION}
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体不一致内容）、fix_suggestion、lens="consistency"。`,
    { label: 'scan:consistency', phase: 'Scan', schema: FINDINGS_SCHEMA }
  ),

  () => agent(
    `面向用户的可见输出使用简体中文。

你是一个代码清理审查员。审查 Claude 全链路中的死代码、过时引用和遗留注释：

**审查范围**：
- core/router-rs/src/hosts/claude_hooks.rs
- core/router-rs/src/hosts/claude_desktop_hooks.rs
- core/router-rs/src/host_integration/mod.rs
- scripts/install-claude.sh
- scripts/install-claude-desktop.sh
- AGENTS_CLAUDE.md
- docs/hosts/claude-desktop.md
- docs/hosts/claude.md
- .claude/CLAUDE.md

**审查维度**：
1. 注释中的 TODO/FIXME/HACK/DEPRECATED
2. 注释掉的代码块（非文档用途）
3. dead code：未使用的函数、变量、导入
4. 过时引用：指向已删除/重命名文件或函数的引用
5. 遗留兼容代码：为旧版本保留但已无用的分支
6. 文档中的"旧版"描述
7. 旧的/多余的注释

**质量要求**：
- 每个 finding 必须引用具体代码行
- evidence 必须是实际代码/注释片段
- 仅报告你有充分证据的问题，不要猜测

${READ_INSTRUCTION}
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体代码/注释片段）、fix_suggestion、lens="dead-code"。`,
    { label: 'scan:dead-code', phase: 'Scan', schema: FINDINGS_SCHEMA }
  ),

  () => agent(
    `面向用户的可见输出使用简体中文。

你是一个测试覆盖率审查员。审查 Claude 全链路的测试覆盖：

**测试文件**：
- core/router-rs/tests/claude_desktop_hooks_tests.rs
- core/router-rs/src/claude_desktop_test_support.rs
- tests/host_integration.rs
- tests/host_platforms.rs
- tests/policy_contracts.rs

**被测代码**：
- core/router-rs/src/hosts/claude_hooks.rs
- core/router-rs/src/hosts/claude_desktop_hooks.rs
- core/router-rs/src/host_integration/mod.rs

**审查维度**：
1. 关键路径是否有测试覆盖
2. 边界条件测试是否充分
3. 测试断言是否足够严格
4. 是否有 test 命中了错误的条件（false positive）
5. mock/stub 是否模拟了真实行为
6. 错误路径的测试覆盖

**质量要求**：
- 每个 finding 必须引用具体测试代码或被测代码
- evidence 必须是实际代码片段
- 仅报告你有充分证据的问题，不要猜测

${READ_INSTRUCTION}
返回 JSON，每个 finding 包含 severity(P0/P1/P2)、title、file、line number range、description、evidence（具体代码片段）、fix_suggestion、lens="test-coverage"。`,
    { label: 'scan:test-coverage', phase: 'Scan', schema: FINDINGS_SCHEMA }
  ),
])

// Collect findings with failure tracking
const LENSES = ['correctness', 'security', 'consistency', 'dead-code', 'test-coverage']
const failedAgents = scanResults.filter(r => !r).length
const allFindings = scanResults.filter(Boolean).flatMap(r => r.findings || [])
log(`扫描完成：${allFindings.length} 个原始 findings，${failedAgents} 个 agent 失败`)

// ── Phase 2: Conservative Merge ──────────────────────────────────────────────

phase('Merge')

const merged = conservativeMerge(allFindings)
log(`保守去重：${allFindings.length} → ${merged.length} 个独立 findings（保留不同 lens 的独立条目）`)

// ── Phase 3: Adversarial Verification ────────────────────────────────────────

phase('Verify')

const verified = await pipeline(
  merged,
  (f, _orig, i) => agent(
    `面向用户的可见输出使用简体中文。

你是一个对立验证员。你的任务是反驳以下 finding。只有当证据充分且你无法找到合理反驳时，才确认为真实问题。

**Finding #${i + 1}**:
- 严重性: ${f.severity}
- 标题: ${f.title}
- 文件: ${f.file}
- 行号: ${f.line || '未指定'}
- 描述: ${f.description}
- 证据: ${f.evidence}
- 审查维度: ${f.lens}
- 修复建议: ${f.fix_suggestion || '未提供'}

**你的验证步骤**：
1. 读取相关源代码，确认 finding 引用的代码确实存在且引用正确
2. 分析该代码的上下文和设计意图
3. 判断：这真的是一个 bug/问题，还是有意设计/可接受的做法？
4. 如果确认为真实问题：
   - 说明根因（root_cause）
   - 给出准确的修复建议
5. 如果是误报，说明为什么不是问题

**重要**：如果你无法读取相关文件（如文件不存在或路径错误），请将 is_real 设为 false 并在 reasoning 中说明原因。

${READ_INSTRUCTION}
返回 JSON，包含 is_real（boolean）、confirmed_severity、root_cause（根因分析）、reasoning（详细推理过程）、fix_suggestion。`,
    { label: `verify:${i}`, phase: 'Verify', schema: VERDICT_SCHEMA }
  )
).catch(err => {
  log(`验证阶段 pipeline 异常: ${err}`)
  return merged.map(() => null)
})

// ── Phase 4: Synthesize ──────────────────────────────────────────────────────

phase('Synthesize')

// Merge verification results with findings
const confirmed = []
const rejected = []

merged.forEach((f, i) => {
  const v = verified[i]
  if (!v) {
    rejected.push({ ...f, rejection_reason: '验证 agent 未返回结果' })
  } else if (!v.is_real) {
    rejected.push({ ...f, rejection_reason: v.reasoning })
  } else {
    confirmed.push({
      ...f,
      severity: v.confirmed_severity || f.severity,
      root_cause: v.root_cause || '',
      fix_suggestion: v.fix_suggestion || f.fix_suggestion,
      verdict: v.reasoning,
    })
  }
})

// Sort by severity
confirmed.sort((a, b) => {
  const order = { P0: 0, P1: 1, P2: 2 }
  return (order[a.severity] ?? 99) - (order[b.severity] ?? 99)
})

// Coverage analysis
const lensesWithConfirmed = [...new Set(confirmed.map(f => f.lens))]

log(`验证完成：${merged.length} 个 findings 中 ${confirmed.length} 个经验证为真实问题，${rejected.length} 个被驳回`)
log(`覆盖度：${LENSES.length} 个维度运行，${lensesWithConfirmed.length} 个有 confirmed findings，${failedAgents} 个 agent 失败`)

return {
  total_scanned: allFindings.length,
  confirmed_count: confirmed.length,
  findings: confirmed,
  rejected_count: rejected.length,
  rejected: rejected,
  coverage: {
    lenses_run: LENSES,
    lenses_with_confirmed: lensesWithConfirmed,
    agents_failed: failedAgents,
  },
}
