export const meta = {
  name: 'deep-function-wiring-audit-v2',
  description: 'Scan each SKILL.md for broken refs, find orphan Rust fns, verify MCP tool impls. Each agent reads files independently.',
  phases: [
    { title: 'Skill-map', detail: 'SKILL.md broken reference scan' },
    { title: 'Code-scan', detail: 'Orphan pub fn scan' },
    { title: 'Cross-ref', detail: 'MCP tool implementation verification' },
    { title: 'Synthesize', detail: 'Compile all findings' },
  ],
}

// The main conversation already read the data and passed it via args.
// args contains:
//   skillSlugs: ["sentry", "goalx", ...]   - 45 skill slugs from routing table
//   toolSlugs: ["goal_state_manage", ...]   - 100 MCP tool slugs from registry
//   serverTools: {"router-rs-framework": [...], ...} - MCP servers and their tool lists
//   serverCrates: {"router-rs-framework": "router-rs", ...} - MCP server -> crate mapping

// If no args, agent() calls will read data independently
var SKILL_SLUGS = args.skillSlugs || []
var TOOL_SLUGS = args.toolSlugs || []
var SERVER_TOOLS = args.serverTools || {}
var SERVER_CRATES = args.serverCrates || {}
var TOOL_DOMAINS = args.toolDomains || {}
var SK_KEYS = args.skKeys || []
var SK_ENTRIES = args.skEntries || []

var info = 'Loaded args: ' + SKILL_SLUGS.length + ' skills from routing, ' + TOOL_SLUGS.length + ' MCP tools'

if (!SKILL_SLUGS.length) {
  info += ' (reading files directly...)'
}

log(info)

// ===============================================================
phase('Skill-map')
log('Scanning SKILL.md files for broken references...')
// ===============================================================

var ski = SKILL_SLUGS
var BATCH = 7
var skillIssues = []

for (var i = 0; i < ski.length; i += BATCH) {
  var batch = ski.slice(i, i + BATCH)
  var batchResults = await Promise.all(batch.map(function(slug) {
    return agent('Read skills/' + slug + '/SKILL.md with the Read tool. Analyze for BROKEN REFERENCES only:\n' +
      '1. CLI commands (cargo run/test, python, npm, npx, etc.)\n' +
      '2. File paths referenced that should exist\n' +
      '3. Tool names that don\'t match valid harness tools (Agent, Read, Write, Edit, Bash, Grep, Glob, WebSearch, WebFetch, NotebookEdit)\n' +
      'Only flag things that clearly DO NOT EXIST or are MISSPELLED.\n' +
      'Severity: "high"=broken CLI command, "med"=missing file ref, "low"=dead link/comment.\n' +
      'IMPORTANT: do NOT flag "framework_goal_drive" or "closeout_gate" or other framework commands — those are stdio-based, not tools.\n' +
      'Return your findings in a "findings" array. Use [] if nothing is broken.',
      {label: 'ski-' + slug, phase: 'Skill-map',
        schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
          skill: {type: 'string'}, type: {type: 'string'},
          detail: {type: 'string'}, severity: {type: 'string'}
        }, required: ['skill', 'type', 'detail', 'severity']}}}, required: ['findings']}})
  }))
  var flat = batchResults.filter(Boolean).map(function(r) { return r.findings || [] }).flat()
  skillIssues.push.apply(skillIssues, flat)
  log('Skills [' + (i+1) + '-' + Math.min(i+BATCH, ski.length) + '/' + ski.length + '] issues=' + flat.length)
}

// ===============================================================
phase('Code-scan')
log('Scanning core crates for orphan pub fns...')
// ===============================================================

var codeIssues = await Promise.all([
  agent('Search in core/research-harness/src/ for:\n' +
    '1. pub fn handle_* or *_handler that has NO matching registered tool slug\n' +
    '2. Dispatch/match arms calling unimplemented!() or todo!()\n' +
    '3. cfg(feature="...") that appears never wired to a tool\n' +
    '4. TODO/FIXME about missing wiring\n' +
    '(Ignore tools already in this list: ' + TOOL_SLUGS.slice(0, 20).join(', ') + ')\n' +
    'Return findings in a "findings" array.',
    {label: 'scan-research', phase: 'Code-scan',
      schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
        path: {type: 'string'}, type: {type: 'string'},
        detail: {type: 'string'}, severity: {type: 'string'}
      }, required: ['path', 'type', 'detail', 'severity']}}}, required: ['findings']}}),
  agent('Search in core/core-state/src/ + core/routing-engine/src/ + core/skill-layer/src/ + core/framework-kernel/src/ for:\n' +
    '1. pub fn handle_* or *_handler with NO matching registered tool\n' +
    '2. Dispatch/match arms calling unimplemented!() or todo!()\n' +
    '3. cfg(feature="...") not wired to a tool\n' +
    'Registered slugs: ' + TOOL_SLUGS.join(', ') + '\n' +
    'Return findings in a "findings" array.',
    {label: 'scan-core', phase: 'Code-scan',
      schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
        path: {type: 'string'}, type: {type: 'string'},
        detail: {type: 'string'}, severity: {type: 'string'}
      }, required: ['path', 'type', 'detail', 'severity']}}}, required: ['findings']}}),
  agent('Search in core/router-rs/src/ for:\n' +
    '1. pub fn handle_* or *_handler with NO matching registered tool\n' +
    '2. Dispatch/match arms calling unimplemented!() or todo!()\n' +
    '3. cfg(feature="...") not wired\n' +
    'Registered slugs: ' + TOOL_SLUGS.join(', ') + '\n' +
    'Return findings in a "findings" array.',
    {label: 'scan-router', phase: 'Code-scan',
      schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
        path: {type: 'string'}, type: {type: 'string'},
        detail: {type: 'string'}, severity: {type: 'string'}
      }, required: ['path', 'type', 'detail', 'severity']}}}, required: ['findings']}}),
  agent('Search in core/session-supervisor/src/ for:\n' +
    '1. pub fn handle_* or *_handler with NO matching registered tool\n' +
    '2. Dispatch/match arms calling unimplemented!() or todo!()\n' +
    '3. cfg(feature="...") not wired\n' +
    '4. TODO/FIXME about missing wiring\n' +
    'Registered slugs: ' + TOOL_SLUGS.join(', ') + '\n' +
    'Return findings in a "findings" array.',
    {label: 'scan-session', phase: 'Code-scan',
      schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
        path: {type: 'string'}, type: {type: 'string'},
        detail: {type: 'string'}, severity: {type: 'string'}
      }, required: ['path', 'type', 'detail', 'severity']}}}, required: ['findings']}}),
])
codeIssues = [].concat.apply([], codeIssues.map(function(r) { return r.findings || [] }))

// ===============================================================
phase('Cross-ref')
log('Verifying MCP tool implementations...')
// ===============================================================

var domains = {}
TOOL_SLUGS.forEach(function(s) {
  var d = TOOL_DOMAINS[s] || 'unknown'
  if (!domains[d]) domains[d] = []
  domains[d].push(s)
})

var domainStr = ''
for (var d in domains) {
  domainStr += '- ' + d + ': ' + domains[d].join(', ') + '\n'
}

var implResult = await agent('Verify MCP tool implementations exist in source code.\n\n' +
  'Tool domains:\n' + domainStr + '\n' +
  'Server -> crate mapping (grep these directories for handle functions or dispatch match arms):\n' +
  '- router-rs-framework -> core/router-rs/src/ OR core/core-state/src/ OR core/routing-engine/src/ OR core/skill-layer/src/\n' +
  '- research -> core/research-harness/src/\n' +
  '- browser -> core/router-rs/src/\n' +
  '- codegraph -> tools/codegraph-rs/src/\n' +
  '- stdio-binary -> tools/rust_tools/X/src/ (X=pdf_tool_rs, ooxml_parser_rs, pptx_tool_rs, financial_data_rs, citation_tool_rs, gh_source_gate_rs)\n\n' +
  'For each tool, find:\n' +
  'a) A handler function (handle_X, process_X, X_handler)\n' +
  'b) A dispatch/match arm containing the tool slug\n' +
  'FLAG: tools where NO implementation found, or impl is clearly stub/todo.\n' +
  'DO NOT flag external tools (paperplain) unless clearly missing.\n' +
  'Return findings in a "findings" array.',
  {label: 'impl-xref', phase: 'Cross-ref',
    schema: {type: 'object', properties: {findings: {type: 'array', items: {type: 'object', properties: {
      tool: {type: 'string'}, type: {type: 'string'},
      detail: {type: 'string'}, severity: {type: 'string'}
    }, required: ['tool', 'type', 'detail', 'severity']}}}, required: ['findings']}}
)
var implIssues = implResult ? implResult.findings || [] : []

// ===============================================================
phase('Synthesize')
log('Compiling final report...')
// ===============================================================

var allIssues = [].concat(
  skillIssues,
  codeIssues.filter(Boolean).flat(),
  implIssues.filter(Boolean).flat()
)

var bySeverity = {}, byCategory = {}
allIssues.forEach(function(i) {
  bySeverity[i.severity] = (bySeverity[i.severity] || 0) + 1
  byCategory[i.type] = (byCategory[i.type] || 0) + 1
})

var skillCounts = {}
skillIssues.forEach(function(i) {
  var s = i.skill || '?'
  skillCounts[s] = (skillCounts[s] || 0) + 1
})

return {
  summary: {
    total: allIssues.length,
    severity: bySeverity,
    category: byCategory,
    skillsWithIssues: skillCounts,
  },
  highSeverity: allIssues.filter(function(i) { return i.severity === 'high' }),
  issues: allIssues,
}
