import { pipeline } from "workflow"
export const meta = {
  name: 'claude-code-cli-audit',
  description: '多agent深度并行审计Claude Code CLI全链路使用问题',
  phases: ['并行审计', '综合报告'],
}

import { agent, parallel, phase, log } from 'workflow'
import { AUDIT_FINDINGS_SCHEMA as FINDINGS_SCHEMA } from './workflow-helpers.js'

const AUDIT_TASKS = [
  {
    name: '架构设计',
    prompt: '审计Claude Code CLI的架构设计问题。检查核心模块划分（router-rs, antigravity, cli）、模块间依赖关系、耦合与循环依赖、扩展点设计。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[{"severity":"critical|high|medium|low","category":"架构设计","title":"...","description":"...","location":"...","impact":"...","recommendation":"..."}],"summary":"..."}'
  },
  {
    name: 'MCP集成',
    prompt: '审计Claude Code的MCP（Model Context Protocol）集成问题。检查MCP服务器配置、工具调用链路、流式响应处理、错误处理和重试机制、状态管理。检查core/router-rs/src/、.claude/mcp.json、skills/目录。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
  {
    name: '权限安全',
    prompt: '审计Claude Code CLI的权限与安全机制。检查Bash/Shell命令执行安全边界、文件系统访问权限、SSRF防护、敏感信息处理、hook机制安全性、配置文件权限。检查cli/antigravity-cli/src/、.claude/settings.json。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
  {
    name: '用户体验',
    prompt: '审计Claude Code CLI的用户体验问题。检查命令行界面一致性、帮助信息和错误提示质量、进度反馈和状态显示、配置复杂度和学习曲线、常见操作的便捷性。检查cli/antigravity-cli/src/、core/router-rs/src/。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
  {
    name: '文档一致性',
    prompt: '审计Claude Code CLI的文档一致性问题。检查文档与代码实现是否一致、README与实际功能是否匹配、AGENTS文档与代码路由是否一致、文档示例是否可运行、版本更新时文档是否同步。检查docs/、README.md、AGENTS.md系列文档。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
  {
    name: '跨平台兼容',
    prompt: '审计Claude Code CLI的跨平台兼容性。检查Unix/Linux/macOS/Windows差异处理、路径处理一致性、Shell兼容性、环境变量处理差异、特殊字符和编码问题。检查cli/antigravity-cli/src/中的平台相关代码。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
  {
    name: '测试覆盖',
    prompt: '审计Claude Code CLI的测试覆盖情况。检查单元测试覆盖、集成测试关键路径覆盖、边界条件和错误处理测试、Mock使用合理性、测试可维护性。检查tests/、core/router-rs/tests/、cli/antigravity-cli/tests/、*_test.rs或test_*.py文件。项目根目录：/Users/joe/Developer/skill。输出JSON: {"findings":[...],"summary":"..."}'
  },
]

phase('阶段一：并行代码审计')
log('启动7个并行agent进行深度代码审计...')

const results = await parallel(AUDIT_TASKS.map(task => () =>
  agent(task.prompt, {
    label: '审计:' + task.name,
    phase: '并行审计',
    schema: FINDINGS_SCHEMA,
  })
))

const allFindings = []
results.forEach(r => {
  if (r && r.findings) {
    r.findings.forEach(f => allFindings.push(f))
  }
})

log('审计完成，共发现 ' + allFindings.length + ' 个问题')

const severityOrder = { critical: 0, high: 1, medium: 2, low: 3 }
allFindings.sort((a, b) => {
  const aOrder = severityOrder[a.severity] || 4
  const bOrder = severityOrder[b.severity] || 4
  return aOrder - bOrder
})

phase('阶段二：生成综合报告')

const critical = allFindings.filter(f => f.severity === 'critical').length
const high = allFindings.filter(f => f.severity === 'high').length
const medium = allFindings.filter(f => f.severity === 'medium').length
const low = allFindings.filter(f => f.severity === 'low').length

const categories = {}
allFindings.forEach(f => {
  categories[f.category] = (categories[f.category] || 0) + 1
})

let detailFindings = ''
allFindings.forEach((f, i) => {
  detailFindings += '\n### ' + (i + 1) + '. [' + f.severity.toUpperCase() + '] ' + f.title + '\n\n'
  detailFindings += '- **类别**: ' + f.category + '\n'
  detailFindings += '- **位置**: `' + f.location + '`\n'
  detailFindings += '- **影响**: ' + f.impact + '\n'
  detailFindings += '- **描述**: ' + f.description + '\n'
  detailFindings += '- **建议**: ' + f.recommendation + '\n'
})

let categoryStats = ''
Object.entries(categories).sort((a, b) => b[1] - a[1]).forEach(([cat, count]) => {
  categoryStats += '- **' + cat + '**: ' + count + ' 个问题\n'
})

const criticalHigh = allFindings.filter(f => f.severity === 'critical' || f.severity === 'high')
const mediumItems = allFindings.filter(f => f.severity === 'medium').slice(0, 5)
const lowItems = allFindings.filter(f => f.severity === 'low').slice(0, 5)

let priority = ''
if (criticalHigh.length > 0) {
  priority += '### 立即处理（Critical/High）\n'
  criticalHigh.forEach(f => {
    priority += '- ' + f.title + ' (' + f.severity + ')\n'
  })
  priority += '\n'
}
if (mediumItems.length > 0) {
  priority += '### 建议近期处理（Medium）\n'
  mediumItems.forEach(f => {
    priority += '- ' + f.title + '\n'
  })
  priority += '\n'
}
if (lowItems.length > 0) {
  priority += '### 可延后处理（Low）\n'
  lowItems.forEach(f => {
    priority += '- ' + f.title + '\n'
  })
}

const nowStr = new Date().toISOString()
const report = '# Claude Code CLI 全链路使用问题审计报告\n\n' +
  '生成时间: ' + nowStr + '\n' +
  '审计维度: ' + AUDIT_TASKS.length + '个\n' +
  '发现问题总数: ' + allFindings.length + '\n\n---\n\n' +
  '## 执行摘要\n\n' +
  '本次审计共发现 ' + allFindings.length + ' 个问题，其中：\n' +
  '- 严重（Critical）: ' + critical + ' 个\n' +
  '- 高优先级（High）: ' + high + ' 个\n' +
  '- 中优先级（Medium）: ' + medium + ' 个\n' +
  '- 低优先级（Low）: ' + low + ' 个\n\n' +
  (critical > 0 ? '⚠️ 发现严重问题，需要立即处理！\n' : '✅ 未发现阻断性严重问题。\n') +
  (high > 2 ? '⚠️ 高优先级问题较多，建议重点关注。\n' : '') +
  '\n---\n\n' +
  '## 详细发现（按严重性排序）\n' +
  detailFindings + '\n---\n\n' +
  '## 按类别统计\n\n' +
  categoryStats + '\n---\n\n' +
  '## 修复优先级建议\n\n' +
  priority

const { mkdirSync, writeFileSync } = await import('fs')
const reportPath = '/Users/joe/Developer/skill/artifacts/current/claude-code-cli-audit-report.md'
try {
  mkdirSync('/Users/joe/Developer/skill/artifacts/current', { recursive: true })
} catch (e) {}
writeFileSync(reportPath, report)

// 暂存报告，由用户手动 commit（安全约束：禁止自动提交）
try {
  const { execSync } = await import('child_process')
  execSync(`git add "${reportPath}"`, { cwd: '/Users/joe/Developer/skill', timeout: 10000 })
  log('报告已暂存，请手动 commit: git commit -m "docs: audit report — claude-code-cli"')
} catch (e) {
  log('报告暂存跳过: ' + (e.message || e))
}

log('综合报告已保存至: ' + reportPath)

return { findings: allFindings, reportPath: reportPath, total: allFindings.length }
