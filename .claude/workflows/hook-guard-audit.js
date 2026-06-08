import { agent, parallel, pipeline, phase, log } from "workflow"
export const meta = {
  name: 'hook-guard-audit',
  description: 'Deep audit of PreToolUse hook path protections — find outdated guards, test, plan optimization',
  phases: [
    { title: 'Investigate', detail: 'multi-angle probe of 4 guard categories' },
    { title: 'Cross-ref', detail: 'check design docs, sync manifests, actual usage' },
    { title: 'Test', detail: 'verify which guards still serve a purpose' },
    { title: 'Plan', detail: 'synthesize optimization recommendations' },
    { title: 'Review', detail: 'adversarial review of the plan' },
  ],
};

// ── Phase 1: Parallel investigation of each guard category ──
phase('Investigate')

const GUARD_CATEGORIES = [
  {
    key: 'generated_entrypoint',
    label: '生成的宿主入口点',
    paths: ['AGENTS.md', 'AGENTS_CLAUDE.md', 'CLAUDE.md', '.claude/CLAUDE.md'],
    question: '这些文件是否仍由 host-entrypoint-sync 自动生成？查看 core/router-rs/src/host_entrypoint_sync.rs 和 host_integration/ 目录。检查：(1) 哪些确实由 Rust sync 流程管理 (2) 哪些已被手动接管 (3) 是否有 AGENTS_CLAUDE.md 的实际生成逻辑还是已废弃 (4) .claude/CLAUDE.md 是否等同于 CLAUDE.md。报告每个文件的当前状态和是否仍需保护。',
  },
  {
    key: 'framework_guarded',
    label: '框架路由/运行时文件',
    paths: ['core/router-rs/**', 'configs/framework/**', 'skills/SKILL_*'],
    question: '这些前缀保护了框架核心。检查：(1) core/router-rs/ 是否真的只通过 Rust 编译流程修改，还是开发者日常也在直接编辑 (2) configs/framework/ 下哪些文件是纯配置可手动编辑、哪些确实需要通过 Rust 流程 (3) skills/SKILL_ROUTING_RUNTIME.json 和 SKILL_MANIFEST.json 是否仍由框架生成或可手动编辑 (4) 保护范围是否过宽（比如 skills/SKILL_ 前缀会误拦 skills/SKILL.md 等非生成文件）。',
  },
  {
    key: 'host_private',
    label: '宿主私有状态',
    paths: ['.claude/settings.json', '.claude/settings.local.json'],
    question: '这两个文件被 is_host_private_path 保护。检查：(1) 为什么策略要求不能直接 Write/Edit 这些文件 (2) 实际上是否有 update-config skill 或其他机制能修改它们 (3) .claude/ 下还有哪些文件属于 hook state（is_repo_claude_hook_state_file）而不是 host private (4) 这个保护是否与用户的开发工作流冲突。',
  },
  {
    key: 'retired_surface',
    label: '已废弃表面',
    paths: ['.codex/hooks.json', '.agents', 'plugins/skill-framework-native/.mcp.json'],
    question: '这些被标记为"已废弃"。检查：(1) .codex/hooks.json 是否还在使用（查看 .codex/ 目录）(2) .agents 目录是否存在、内容是什么 (3) plugins/skill-framework-native/ 是否仍存在于仓库中 (4) 这些"废弃"路径是否还需要防恢复保护，还是可以完全移除。',
  },
]

const investigations = await parallel(GUARD_CATEGORIES.map(cat => () =>
  agent(
    `面向用户的可见输出使用简体中文。\n\n深度调查 router-rs PreToolUse hook 中「${cat.label}」类别的路径保护。\n\n受保护路径：${JSON.stringify(cat.paths)}\n\n具体问题：${cat.question}\n\n请搜索相关源码、配置文件、设计文档，给出每个路径/前缀的详细状态报告。用 bash grep/find 搜索，不要猜测。报告格式：\n- 路径/前缀\n- 当前状态（活跃/废弃/手动接管）\n- 保护必要性评估（必须保留/可移除/可放宽）\n- 证据（文件路径、代码引用）`,
    { label: `investigate:${cat.key}`, phase: 'Investigate' }
  )
))

// ── Phase 2: Cross-reference with design docs and sync manifests ──
phase('Cross-ref')

const crossRef = await parallel([
  () => agent(
    `面向用户的可见输出使用简体中文。\n\n检查 host-entrypoint-sync 的实际实现：\n1. 搜索 host_entrypoint_sync.rs、host_integration/mod.rs 中关于 AGENTS.md、AGENTS_CLAUDE.md、CLAUDE.md 生成逻辑的代码\n2. 检查 host_integration/projection.rs 中的投影机制\n3. 查看 GENERATED_ENTRYPOINT_PATHS_CLAUDE 和 GENERATED_ARTIFACT 相关定义\n4. 确认哪些入口点文件确实由 sync 流程管理，哪些已被废弃\n5. 检查是否有 sync manifest 文件记录了哪些文件需要同步`,
    { label: 'crossref:entrypoint-sync', phase: 'Cross-ref' }
  ),
  () => agent(
    `面向用户的可见输出使用简体中文。\n\n检查 hook_policy.rs 和 closeout_enforcement.rs 中的路径保护机制：\n1. hook_policy.rs 中的 protected-path 操作和 SETTINGS_GUARDED_PATHS 的关系\n2. closeout_enforcement.rs 是否有独立的路径保护\n3. review_gate_engine.rs 和 review_gate.rs 中对 settings 文件的依赖\n4. 查找所有引用 SETTINGS_GUARDED_PATHS_CLAUDE、GENERATED_ENTRYPOINT_PATHS_CLAUDE 的代码位置\n5. 确认这些常量是否在多处被引用（不只是 PreToolUse hook）`,
    { label: 'crossref:policy-engines', phase: 'Cross-ref' }
  ),
  () => agent(
    `面向用户的可见输出使用简体中文。\n\n外部调研：Claude Code 官方对 .claude/ 目录结构的规范是什么？\n1. 搜索 web 了解 Claude Code 的 settings.json / settings.local.json 的官方用法\n2. 查看 .claude/ 下的 hook state 文件（REVIEW_GATE_STATE.json、TOUCH_STATE.json 等）是否是 router-rs 私有的还是 Claude Code 原生的\n3. 确认 AGENTS.md 是否是 Claude Code 官方要求的文件还是框架自定义的\n4. 检查 CLAUDE.md 的层级结构（global → project）在 Claude Code 中的官方文档`,
    { label: 'crossref:external-specs', phase: 'Cross-ref' }
  ),
])

// ── Phase 3: Test current behavior ──
phase('Test')

const testResults = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '测试 PreToolUse hook 的四类保护在当前环境下的实际行为。\n\n' +
  '注意：当前 .claude/router-rs-hook.env 中已设置 ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD=1，所以需要临时关闭它来测试。\n\n' +
  '测试方法：对每类保护路径，用如下格式调用 router-rs binary（临时 unset ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD 或设为 0）：\n' +
  'echo JSON_PAYLOAD | ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD=0 router-rs host claude hook --event=PreToolUse --repo-root /Users/joe/Developer/skill\n\n' +
  'JSON_PAYLOAD 格式为 {"tool_name":"Read","tool_input":{"file_path":"TARGET_PATH"}}，其中 TARGET_PATH 替换为要测试的路径。\n\n' +
  '测试清单：\n' +
  '1. generated entrypoint: AGENTS.md, AGENTS_CLAUDE.md, CLAUDE.md, .claude/CLAUDE.md\n' +
  '2. framework guarded: configs/framework/test.json, skills/SKILL_ROUTING_RUNTIME.json, skills/SKILL.md\n' +
  '3. host private: .claude/settings.json, .claude/settings.local.json\n' +
  '4. retired surface: .codex/hooks.json, .agents, plugins/skill-framework-native/.mcp.json\n' +
  '5. 边界: skills/SKILL.md（是否被 skills/SKILL_ 前缀误拦）, .claude/workflows/xxx.js（是否被 host private 误拦）\n\n' +
  'binary 路径: /Users/joe/Developer/skill/core/router-rs/target/debug/router-rs\n\n' +
  '记录每个测试的实际返回 JSON，标注哪些是合理的拦截、哪些是误拦。',
  { label: 'test:guard-behavior', phase: 'Test' }
)

// ── Phase 4: Synthesize optimization plan ──
phase('Plan')

const plan = await agent(
  `面向用户的可见输出使用简体中文。\n\n基于以下调查结果，综合制定 PreToolUse hook 路径保护的优化方案。\n\n## 调查结果汇总\n\n### 1. 生成的宿主入口点\n${investigations[0]}\n\n### 2. 框架路由/运行时文件\n${investigations[1]}\n\n### 3. 宿主私有状态\n${investigations[2]}\n\n### 4. 已废弃表面\n${investigations[3]}\n\n### 交叉引用\n- entrypoint-sync: ${crossRef[0]}\n- policy-engines: ${crossRef[1]}\n- external-specs: ${crossRef[2]}\n\n### 测试结果\n${testResults}\n\n## 要求\n\n输出一个结构化的优化方案，包含：\n\n1. **可移除的保护**：哪些路径/前缀的保护已经过时，可以完全删除\n2. **可放宽的保护**：哪些保护范围过宽，应该收窄\n3. **必须保留的保护**：哪些仍然关键，不能动\n4. **新增建议**：是否需要新增缺失的保护\n5. **实现步骤**：具体的代码修改计划（涉及哪些文件、哪些函数、改什么）\n6. **风险评估**：每个改动的风险和回退方案\n\n格式化为 markdown 表格 + 清晰的步骤列表。`,
  { label: 'plan:synthesize', phase: 'Plan' }
)

// ── Phase 5: Adversarial review ──
phase('Review')

const review = await parallel([
  () => agent(
    `面向用户的可见输出使用简体中文。\n\n你是一个严格的安全审计员。审查以下 PreToolUse hook 优化方案，尝试找出任何可能导致安全漏洞的改动。\n\n优化方案：\n${plan}\n\n审查要点：\n1. 移除某个保护后，是否有攻击路径可以利用？\n2. 放宽保护范围后，边界 case 是否安全？\n3. 是否存在隐含的依赖（其他 hook 或流程假设这些保护存在）？\n4. my-light 模式下的安全性是否受影响？\n5. 跨宿主（Cursor/Codex）是否受影响？\n\n对每个优化建议给出：安全通过 / 需要修改 / 危险不可行 的判定。`,
    { label: 'review:security', phase: 'Review' }
  ),
  () => agent(
    `面向用户的可见输出使用简体中文。\n\n你是一个务实的开发者体验审查员。审查以下 PreToolUse hook 优化方案，从开发者日常使用角度评估。\n\n优化方案：\n${plan}\n\n审查要点：\n1. 改动后开发者的工作流是否更顺畅？\n2. 是否有"改了反而更麻烦"的场景？\n3. ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD 这个"大开关"是否合理，还是需要更细粒度的控制？\n4. 与 update-config skill 的交互是否正常？\n5. 新开发者上手时是否会被剩余的保护搞困惑？\n\n对每个建议给出：DX 改善 / 中性 / DX 退化 的判定。`,
    { label: 'review:developer-experience', phase: 'Review' }
  ),
])

return {
  investigations,
  crossRef,
  testResults,
  plan,
  review,
}
