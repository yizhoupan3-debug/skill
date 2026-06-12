export const meta = {
  name: 'full-closeout-audit',
  description: '16维度×12+agent全量审计、修复、清理、合并推送 — 仓库大面积升级后收尾',
  phases: [
    { title: 'Scan', detail: '16 维度全量审计（commit/harness/文档/死代码等）' },
    { title: 'Merge', detail: '合并 findings、修复规划与分支合并' },
    { title: 'Verify', detail: '验证发现真实性并审查最终 diff' },
    { title: 'Synthesize', detail: '执行修复、清理、commit 与 push' },
  ],
}

const FINDING = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          file: { type: 'string' },
          severity: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
          category: { type: 'string' },
          description: { type: 'string' },
        },
        required: ['title', 'severity', 'description'],
      },
    },
  },
  required: ['findings'],
}

const VERDICT = {
  type: 'object',
  properties: {
    isReal: { type: 'boolean' },
    confidence: { type: 'number' },
    evidence: { type: 'string' },
    fix: { type: 'string' },
  },
  required: ['isReal'],
}

const PLAN = {
  type: 'object',
  properties: {
    fixes: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          files: { type: 'array', items: { type: 'string' } },
          priority: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
          approach: { type: 'string' },
        },
        required: ['title', 'files', 'priority'],
      },
    },
  },
  required: ['fixes'],
}

// ═══════════════════════════════════════════════════════
// Phase 1: Commit Review (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase1-CommitReview')

await agent(
  `面向用户的可见输出使用简体中文。
运行 git log --oneline --since="2026-05-27" --format="%h %ai %s"，将全部commit按类型分组统计（安全/测试/文档/重构/功能/其他），每组数量和影响范围。输出JSON分组统计。`,
  { label: 'commit:分类统计', phase: 'Phase1-CommitReview' }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周安全commit。运行:
git log --oneline --since="2026-05-27" --grep="security"
逐一 git show <hash> 审查: 路径遍历防护完整性、fsync对齐、遗漏攻击面、新安全风险。`,
  { label: 'commit:安全审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周测试commit。运行:
git log --oneline --since="2026-05-27" --grep="test"
逐一 git show <hash>: 删除测试是否误删、新测试是否覆盖正确场景、是否掩盖底层bug。`,
  { label: 'commit:测试审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周文档commit。运行:
git log --oneline --since="2026-05-27" --grep="doc"
逐一 git show <hash>: 引用路径正确性、断裂交叉引用、模板字段名、宿主特化泄露。`,
  { label: 'commit:文档审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周重构commit。运行:
git log --oneline --since="2026-05-27" --grep="refactor"
逐一 git show <hash>: 模块拆分接口兼容、遗漏文件移动、引用更新、循环依赖。重点: host_integration拆分、skills重构、lifecycle简化。`,
  { label: 'commit:重构审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周功能commit。运行:
git log --oneline --since="2026-05-27" --grep="feat"
逐一 git show <hash>: 配套测试/文档、新外部依赖、安全影响。`,
  { label: 'commit:功能审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查fixup commit。运行:
git log --oneline --since="2026-05-27" --grep="fixup"
逐一 git show <hash>: 修补完整性、遗留TODO、同类遗漏。`,
  { label: 'commit:fixup审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查合并commit。运行:
git log --oneline --since="2026-05-27" --merges
检查合并干净度、冲突残留、编译问题。
git branch -r --no-merged main 检查未合并远程内容。`,
  { label: 'commit:合并审计', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
分析commit时序逻辑。运行:
git log --oneline --since="2026-05-27" --format="%h %ai %s"
检查: 逻辑顺序断裂、依赖关系、fixup是否紧跟原始commit、安全修复是否在最前。`,
  { label: 'commit:时序分析', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查commit message质量。运行:
git log --since="2026-05-27" --format="%s"
检查: 是否遵循conventional commits、scope是否准确、描述是否充分。`,
  { label: 'commit:消息质量', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查本周变更的影响范围。运行:
git diff --stat HEAD~50..HEAD 2>/dev/null || git log --oneline --since="2026-05-27" | head -50 | while read hash rest; do git show --stat --format="" $hash; done | sort | uniq -c | sort -rn | head -30
识别变更最频繁的文件和模块，评估变更集中度。`,
  { label: 'commit:影响范围', phase: 'Phase1-CommitReview', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查变更完整性: 本次大面积升级中是否有被遗忘的子系统。
检查: core/framework-core/ 是否同步更新、evals/ 是否适配、templates/ 是否同步。`,
  { label: 'commit:变更完整性', phase: 'Phase1-CommitReview', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 2: Harness Audit (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase2-HarnessAudit')

await agent(
  `面向用户的可见输出使用简体中文。
审计 core/router-rs/src/ 核心类型和入口: lib.rs, main.rs, types.rs, harness_contract.rs, harness_context_signals.rs, harness_operator_nudges.rs。检查类型一致性、错误处理、文档注释。`,
  { label: 'harness:核心类型', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计路由系统 core/router-rs/src/route/ 全部文件: routing.rs, scoring.rs, scoring_config.rs, fuzzy.rs, policy.rs, signals.rs, aliases.rs, constants.rs, gate_hints.rs, nl_route_adjustments.rs, skill_record.rs, records.rs, eval.rs, text.rs, types.rs。检查路由算法、scoring权重、policy覆盖、类型一致性。`,
  { label: 'harness:路由系统', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计hook系统: hook_common/(mod.rs, timing.rs, observation.rs, outbound.rs, path_guard.rs), hook_policy/(mod.rs, mcp_safety.rs, bash_guard.rs), hosts/claude_code_hooks.rs(pre,post,stop,user_prompt,session).rs, hosts/cursor_hooks/全部, hosts/codex_hooks/全部, hosts/opencode_agent.rs, hosts/hook_state_common.rs。检查: PreToolUse/PostToolUse/Stop/UserPromptSubmit全覆盖、advisory模式不阻断、guard逻辑、保护列表完整。`,
  { label: 'harness:Hook系统', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计宿主集成: host_integration/mod.rs, artifacts.rs, projection.rs, roots.rs, framework_host_targets.rs, host_entrypoint_sync.rs。检查模块拆分接口、竞态风险、投影逻辑、路径遍历。`,
  { label: 'harness:宿主集成', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计closeout/RFV/ship: closeout_enforcement.rs, rfv_loop/, ship_readiness.rs, execution_contract.rs。检查my-light advisory、RFV状态机、ship判定、边缘case。`,
  { label: 'harness:Closeout+RFV', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计framework runtime: framework_runtime/ 全部文件(mod.rs, types.rs, constants.rs, runtime_view.rs, repo_roots.rs, session_artifacts.rs, framework_doctor.rs, prompt_compression.rs, statusline.rs, json_io.rs, json_value.rs, alias.rs, codex_hooks_duplicate.rs)。检查CAS逻辑、doctor输出、compression数据丢失、重复代码。`,
  { label: 'harness:Runtime', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计skills系统: framework_skills.rs, skill_repo.rs。检查skill加载、缓存一致性、SKILL.md frontmatter与路由匹配、废弃skill未归档。`,
  { label: 'harness:Skills', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计review gate: review/(mod.rs, engine.rs, output_lint.rs, routing_signals.rs, heterogeneous.rs)。检查advisory模式不阻断、评分逻辑、输出格式、路由信号一致性。`,
  { label: 'harness:ReviewGate', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计CLI: cli/mod.rs, args.rs, common.rs, dispatch.rs, runtime_ops.rs。检查参数解析完整性、子命令dispatch覆盖、错误处理。`,
  { label: 'harness:CLI', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计browser MCP: browser_mcp/全部(mod.rs, tests.rs, frag_01_through_types.rs, frag_impl_browser_runtime.rs, frag_impl_cdp.rs, frag_rest.rs)。检查片段拆分接口、CDP协议、资源泄漏、测试覆盖。`,
  { label: 'harness:BrowserMCP', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计misc模块: autopilot_goal.rs, background_state.rs, eval_route.rs, formal_toolchain.rs, framework_profile.rs, framework_maint.rs, router_env_flags.rs, router_rs_observation.rs, router_self.rs, runtime_envelope_ids.rs, runtime_registry/mod.rs, runtime_storage.rs, schema_drift.rs, session_call_tracker.rs, session_supervisor.rs, stdio_transport.rs, task_command.rs, test_env_sync.rs, trace_runtime.rs, utils/全部。检查死函数、未处理错误、类型不一致。`,
  { label: 'harness:杂项模块', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计framework-core核心: core/framework-core/src/全部(lib.rs, rfv_loop.rs, state_manager/(mod.rs, goal_state.rs, rfv_state.rs, task_pointers.rs, evidence.rs, hook_text_utils.rs, verification_boundary.rs), step_ledger.rs, task_ledger.rs, task_state.rs, task_state_aggregate.rs, types/, utils/, math_verify/)。检查与router-rs的集成、竞态安全、路径防护。`,
  { label: 'harness:Antigravity', phase: 'Phase2-HarnessAudit', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 3: Doc Audit (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase3-DocAudit')

await agent(
  `面向用户的可见输出使用简体中文。
审计AGENTS.md系列(AGENTS.md, AGENTS_CLAUDE.md, AGENTS_CURSOR.md, AGENTS_CODEX.md, AGENTS_ANTIGRAVITY.md, AGENTS_OPENCODE.md): 共用vs宿主分界、交叉引用、简体中文、宿主特化泄露。`,
  { label: 'doc:AGENTS系列', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计主文档: README.md, MIGRATION.md, RTK.md。检查架构描述准确性、过期迁移步骤、命令/路径正确性、断裂链接。`,
  { label: 'doc:主文档', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计docs/主要真源(docs/README.md, docs/harness_architecture/index.md, docs/harness_architecture/03-hook-and-switches.md, docs/harness_policy_map.md, docs/host_adapter_contract.md, docs/runtime_unified_spec.md, docs/framework_profile_contract.md, docs/references/AGENTS_OPERATOR_SURFACE.md, docs/closeout_enforcement.md, docs/framework_naming_conventions.md, docs/framework_operator_primer.md, docs/operations/index.md, docs/rfv_loop_harness.md, docs/rust_contracts.md, docs/task_state_unified_resolve.md, docs/architecture/security.md, docs/plans/README.md, README.md)。逐文件检查内容与代码一致性、过期设计、路径引用、重复内容。`,
  { label: 'doc:docs目录', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计Claude Desktop配置: .claude/CLAUDE.md, .claude/mcp.README.md, .claude/mcp.json, .claude/settings.json, .claude/settings.local.json。检查MCP工具列表一致性、服务器配置、权限配置、敏感信息。`,
  { label: 'doc:Claude配置', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计其他宿主配置: .cursor/config.toml, .cursor/hooks.json, .codex/config.toml, .codex/hooks.json, .codex/README.md, .codex/host_entrypoints_sync_manifest.json, .opencode/opencode.json, .opencode/.framework-projection.json, .gemini/全部。检查与框架契约一致、hooks命令路径、projection匹配、配置漂移。`,
  { label: 'doc:宿主配置', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计.claude/rules/framework.md: 指令覆盖、冲突指令、MCP工具描述准确性、lifecycle profile、联网约束、host-specific混入。`,
  { label: 'doc:rules', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计模板文件 templates/ 全部: 模板变量与代码匹配、过期模板、宿主特化内容。`,
  { label: 'doc:模板', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计artifacts/: artifacts/下所有.md和.json。过期审计报告、可归档历史产物、准确性、敏感信息。`,
  { label: 'doc:artifacts', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计JSON配置: configs/scoring_weights.json, .mcp.json, .gemini/mcp.json。JSON格式合法性、字段与Rust类型一致、权重合理性、冗余。`,
  { label: 'doc:JSON配置', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
文档vs代码一致性比对:
AGENTS.md vs docs/harness_architecture/index.md vs docs/runtime_unified_spec.md
docs/framework_profile_contract.md vs core/router-rs/src/framework_profile.rs
docs/closeout_enforcement.md vs closeout_enforcement.rs
docs/harness_architecture/03-hook-and-switches.md §6 vs hook相关源码
docs/host_adapter_contract.md vs host_integration源码
docs/rust_contracts.md vs Rust 实现`,
  { label: 'doc:文档vs代码', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计.pyproject.toml, Cargo.toml, Justfile中的文档/注释质量。检查过期说明、缺失注释、不一致描述。`,
  { label: 'doc:项目配置', phase: 'Phase3-DocAudit', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计 .claude/mcp.README.md 和 docs/rust_contracts.md / docs/references/AGENTS_OPERATOR_SURFACE.md 的 MCP 工具描述是否与 .mcp.json 实际配置一致。`,
  { label: 'doc:MCP文档', phase: 'Phase3-DocAudit', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 4: Dead Code (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase4-DeadCode')

await agent(
  `面向用户的可见输出使用简体中文。
Rust编译器死代码警告: cargo build 2>&1 | grep -i "warning.*dead\\|warning.*unused"。收集并分析。`,
  { label: 'deadcode:编译警告', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
搜索unreachable/todo/unimplemented/panic宏: grep -rn "unreachable!\\|todo!\\|unimplemented!\\|panic!(.*not implemented" core/ src/ cli/ --include="*.rs"`,
  { label: 'deadcode:宏标记', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
搜索未使用pub函数: 用grep列出core/router-rs/src/中所有pub fn定义，然后对每个函数名grep全仓库检查是否有调用。报告无调用的函数。`,
  { label: 'deadcode:死pub函数', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
扫描skills废弃项: 对比skills/目录与.archive-cold/。检查每个活跃skill是否被路由引用。检查skill目录中未引用的脚本/配置。`,
  { label: 'deadcode:废弃skill', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
扫描scripts/死代码: 逐一检查scripts/*.sh。是否有引用已不存在路径/工具的脚本、从未被文档引用的脚本、功能重复的脚本。`,
  { label: 'deadcode:Scripts', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Python死代码: 检查pyproject.toml依赖使用情况、evals/过期脚本、.py文件未使用import/函数。`,
  { label: 'deadcode:Python', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
配置死代码: .claude/settings*.json不再生效项、.cursor/config.toml过期选项、.codex/config.toml过期选项、Cargo.toml未使用依赖、.cargo/config.toml必要性。`,
  { label: 'deadcode:配置', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
死文档: docs/中描述已删除功能的文档、artifacts/过期产物、从未被引用的.md文件。`,
  { label: 'deadcode:死文档', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Rust未使用导入: cargo clippy -- -W clippy::unused-imports 2>&1。同时检查冗余closure和未使用变量。`,
  { label: 'deadcode:Clippy', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
测试死代码: tests/中被ignore的测试、从未调用的测试函数、测试工具未使用部分、#[cfg(test)]中死代码。`,
  { label: 'deadcode:测试', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
rust_tools/死代码: 检查rust_tools/Cargo.toml和源码，未使用依赖、死代码、与core/router-rs功能重复。`,
  { label: 'deadcode:RustTools', phase: 'Phase4-DeadCode', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
二进制产物和临时文件: find . -name "*.pyc" -o -name "__pycache__" -o -name ".DS_Store" -o -name "*.tmp" -o -name "*.bak" -o -name "*.swp" | grep -v .git/ | grep -v .venv/ | grep -v target/。同时检查.test-output.log和target/大小。`,
  { label: 'deadcode:二进制', phase: 'Phase4-DeadCode', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 5: Expired Design (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase5-ExpiredDesign')

await agent(
  `面向用户的可见输出使用简体中文。
扫描docs/过期设计: 逐一读取docs/下所有.md。检查是否描述已不存在的模块/功能、设计决策已被替代、架构图与当前结构不一致、过期TODO/FIXME、引用已删除文件/函数。`,
  { label: 'expired:设计文档', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Rust过期设计模式: grep -rn "TODO\\|FIXME\\|XXX\\|HACK\\|deprecated\\|DEPRECATED" core/router-rs/src/。检查废弃feature flags、过期trait实现、deprecated函数仍被使用。`,
  { label: 'expired:Rust设计', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Skills过期设计: 检查活跃skill SKILL.md引用已不存在工具/宿主、描述流程与实际不符、归档skill仍被引用、依赖关系断裂。`,
  { label: 'expired:Skills设计', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Hook过期设计: .githooks/过期hook、hooks.json引用已不存在脚本、hook_policy/与当前lifecycle不一致、废弃事件处理。`,
  { label: 'expired:Hook设计', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
配置过期设计: Cargo.toml edition/features、pyproject.toml Python版本、.rtk/filters.toml有效性、.github/dependabot.yml、.vscode/settings.json。`,
  { label: 'expired:配置设计', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
生命周期设计过期: discussx→planx→implementx→verifyx是否完整、各skill中lifecycle描述与实际一致、goal_state/closeout行为与文档一致、RFV循环与代码匹配。读取skills/discussx/SKILL.md, planx/SKILL.md, implementx/SKILL.md, verifyx/SKILL.md, closeout_enforcement.rs, rfv_loop/, execution_contract.rs。`,
  { label: 'expired:生命周期', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
宿主适配器过期: host_adapter_contract.md与当前宿主列表一致、projection配置过期、runtime_registry遗漏宿主、引用已删除API。`,
  { label: 'expired:宿主适配', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
安全设计过期: web_fetch_guard.rs保护列表覆盖、hook_common/outbound.rs保护列表、路径遍历防护覆盖新文件操作入口、安全环境变量文档完整。`,
  { label: 'expired:安全设计', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Schema过期: evals/SCHEMA.json与实际评估格式、JSON schema覆盖输出格式、MCP工具schema与参数、schema_drift.rs检测有效性。`,
  { label: 'expired:Schema', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
投影配置过期: 读取全部.framework-projection*.json(6个文件)。各投影与宿主能力一致、引用已不存在路径、字段与framework_profile.rs匹配。`,
  { label: 'expired:投影', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
GIT hygiene过期: 检查docs/operations/index.md、.github/配置、.githooks/内容是否与当前实践一致。`,
  { label: 'expired:Git规范', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
跨文件设计一致性: docs/rust_contracts.md vs Rust实现、docs/rfv_loop_harness.md vs rfv_loop/、docs/task_state_unified_resolve.md vs task_command.rs/execution_contract.rs。`,
  { label: 'expired:跨文件一致', phase: 'Phase5-ExpiredDesign', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 6: Test Coverage (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase6-TestCoverage')

await agent(
  `面向用户的可见输出使用简体中文。
运行cargo test收集结果: cargo test 2>&1 | tail -30。统计通过/失败/ignored数量。`,
  { label: 'test:Cargo测试', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计宿主集成测试: tests/host_integration.rs, tests/host_platforms.rs。覆盖所有宿主？正面+负面？遗漏场景？外部依赖？`,
  { label: 'test:宿主集成', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计策略测试: tests/policy/registry_review_gate.rs, tests/policy_contracts.rs, tests/policy_cursor_rules_links.rs, tests/policy_markdown_links.rs, tests/policy_registry_review_gate.rs。覆盖所有policy？测试数据反映真实场景？过期断言？隐式依赖？`,
  { label: 'test:策略测试', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
统计Rust单元测试: grep -rn "#[test]" core/router-rs/src/ | wc -l。按模块统计test函数数量，识别无测试模块。`,
  { label: 'test:单元统计', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计browser MCP测试: core/router-rs/src/browser_mcp/tests.rs, tests/browser_mcp_scripts.rs。核心操作覆盖？mock/真实CDP切换？独立运行？边界条件？`,
  { label: 'test:BrowserMCP', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计契约测试: tests/documentation_contracts.rs, tests/patch_script_contracts.rs, tests/tracked_markdown_utf8_contract.rs。文档契约覆盖？脚本契约验证？UTF8覆盖？新增契约需求？`,
  { label: 'test:契约测试', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计hook测试: hosts/cursor_hooks/tests.rs, browser_mcp/tests.rs, tests/下hook相关。hook系统测试覆盖评估。`,
  { label: 'test:Hook测试', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计测试基础设施: tests/common/mod.rs, tests/common/review_gate_lanes.rs。helper使用、缺失fixture、死代码。`,
  { label: 'test:基础设施', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
识别缺失测试场景: closeout_enforcement advisory模式测试、rfv_loop状态机测试、session_artifacts CAS并发测试、prompt_compression效果测试、host_entrypoint_sync同步测试。描述每个缺失测试的内容和重要性。`,
  { label: 'test:缺失场景', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计test_env_sync.rs: 测试环境同步机制是否正确、是否覆盖所有环境变量、是否安全。`,
  { label: 'test:环境同步', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计evals/: evals/SCHEMA.json、evals/目录下所有评估脚本。评估覆盖度和质量。`,
  { label: 'test:Evals', phase: 'Phase6-TestCoverage', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
回归测试检查: 最近50个commit修复的bug是否都有回归测试。特别关注安全修复和测试修复commit。`,
  { label: 'test:回归测试', phase: 'Phase6-TestCoverage', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 7: Host-Specific Audit (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase7-HostSpecific')

await agent(
  `面向用户的可见输出使用简体中文。
扫描共用文件宿主特化: AGENTS.md(共用内核)是否包含Claude Desktop MCP工具名/Cursor配置路径/特定宿主生命周期行为。docs/共用文档是否声称"仅适用于X宿主"。`,
  { label: 'host:共用扫描', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Claude Desktop特化: .claude/CLAUDE.md不该有此内容？mcp.json正确反映CD能力？settings权限合理？settings.local敏感信息？特化仅在.claude/？`,
  { label: 'host:Claude Desktop', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Claude Code CLI特化: .claude/rules/framework.md正确反映CLI能力？CLI特有环境变量文档化？bootstrap脚本正确？CLI特有行为混入共用代码？`,
  { label: 'host:Claude Code', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Cursor特化: .cursor/config.toml完整？hooks.json正确？AGENTS_CURSOR.md仅含Cursor特有？bootstrap正确？cursor_hooks代码隔离？`,
  { label: 'host:Cursor', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Codex特化: .codex/config.toml完整？hooks.json正确？README准确？host_entrypoints_sync_manifest完整？AGENTS_CODEX.md仅含Codex特有？codex_hooks隔离？`,
  { label: 'host:Codex', phase: 'Phase7-HostSpecific', schema: FINDING }
)
// antigravity host audit removed (host retired)

await agent(
  `面向用户的可见输出使用简体中文。
OpenCode特化: .opencode/opencode.json完整？framework-projection正确？AGENTS_OPENCODE.md仅含特有？opencode_agent.rs正确？`,
  { label: 'host:OpenCode', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Gemini特化: .gemini/mcp.json完整？settings.json正确？缺少AGENTS_GEMINI.md？`,
  { label: 'host:Gemini', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
投影漂移: 读取6个.framework-projection*.json。同一宿主不同projection一致？有宿主缺projection？字段与framework_profile.rs一致？引用已删除功能？`,
  { label: 'host:投影漂移', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
交叉污染: hosts/中各宿主hook互相引用？宿主特定工具函数放在共用模块？framework_host_targets.rs正确区分？hooks.json引用其他宿主脚本？`,
  { label: 'host:交叉污染', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
共享Rust代码宿主检查: src/lib.rs和core/中共享代码是否有宿主特化cfg/条件逻辑。#[cfg(feature="host-X")]是否放在了共用库中。`,
  { label: 'host:Rust共享', phase: 'Phase7-HostSpecific', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
共享skill宿主检查: skills/中所有skill的SKILL.md是否嵌入特定宿主逻辑(if host=X则...)。共享skill的hooks/scripts是否包含宿主特化路径。`,
  { label: 'host:Skill共享', phase: 'Phase7-HostSpecific', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 8: Ops Doc (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase8-OpsDoc')

await agent(
  `面向用户的可见输出使用简体中文。
优化安装文档: scripts/install-*.sh, scripts/*-bootstrap-*.sh, README.md安装说明。统一安装流程、多平台注意事项、故障排除、版本兼容矩阵。`,
  { label: 'ops:安装文档', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
多端同步方案: 各宿主配置可共享(dotfiles同步)？多台机器框架配置同步？Git hook每台机器重复？Skills同步？编写最佳实践建议。`,
  { label: 'ops:多端同步', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
新宿主接入指南: host_adapter_contract.md接入指南清晰？添加新宿主step-by-step？接口契约清晰？framework_host_targets模板？`,
  { label: 'ops:新宿主接入', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
故障排除文档: 统一故障排除？framework_doctor输出文档化？unblock脚本使用说明？常见错误模式和解决方案。`,
  { label: 'ops:故障排除', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
日常运维文档: docs/operations/index.md 完整？运维checklist？技能更新流程？框架升级流程？备份策略？`,
  { label: 'ops:日常运维', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计framework_operator_primer.md: 覆盖所有运维场景？与当前代码一致？内容组织和可读性。`,
  { label: 'ops:操作手册', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计naming_conventions.md: 代码遵循？未覆盖命名场景？更新规范覆盖新模块。`,
  { label: 'ops:命名规范', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计harness文档: docs/harness_architecture/index.md, docs/harness_policy_map.md, docs/harness_architecture/03-hook-and-switches.md。是否需更新反映本周变更？优化结构和可读性。`,
  { label: 'ops:Harness文档', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审计profile契约文档: docs/framework_profile_contract.md, docs/references/AGENTS_OPERATOR_SURFACE.md §Operator profiles。与 framework_profile.rs 一致？my/my-light 描述准确？`,
  { label: 'ops:Profile契约', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
安全运维文档: 安全配置(路径保护、权限控制)说明完整？安全事件响应流程？安全审计日志？`,
  { label: 'ops:安全运维', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
性能运维文档: 缓存策略说明？路由优化说明？性能监控方法？性能退化排查步骤？`,
  { label: 'ops:性能运维', phase: 'Phase8-OpsDoc', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
CI/CD文档: GitHub Actions配置？发布流程？dependabot处理？分支保护规则？`,
  { label: 'ops:CI/CD', phase: 'Phase8-OpsDoc', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 9: Adversarial Testing (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase9-AdversarialTest')

await agent(
  `面向用户的可见输出使用简体中文。
构建对抗测试: cargo build 2>&1, cargo build --all-features 2>&1, cargo build --no-default-features 2>&1, cargo clippy -- -D warnings 2>&1。记录所有构建问题。`,
  { label: 'adversarial:构建', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
路径遍历对抗: session_artifacts.rs文件操作、host_integration/artifacts.rs路径拼接、roots.rs路径解析、utils/path_guard.rs防护。尝试构造../../../etc/passwd、符号链接、unicode混淆。`,
  { label: 'adversarial:路径遍历', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
并发对抗: session_artifacts.rs CAS竞态、runtime_storage.rs读写竞态、session_call_tracker.rs跟踪竞态、session_supervisor.rs管理竞态、background_state.rs状态竞态。检查RwLock、原子操作、死锁风险。`,
  { label: 'adversarial:并发', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
错误处理对抗: 扫描unwrap()导致panic风险、expect()消息质量、未处理Result、库代码中panic!宏。grep -rn "unwrap()\\|expect(\\|panic!(" core/router-rs/src/ | grep -v test。`,
  { label: 'adversarial:错误处理', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
输入验证对抗: MCP工具参数类型范围验证、JSON输入schema验证、路由查询长度格式限制、hook输入边界检查、CLI参数注入防护。`,
  { label: 'adversarial:输入验证', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
性能瓶颈: 路由scoring O(n²)+、prompt_compression不必要字符串拷贝、JSON序列化优化、文件I/O同步、不必要clone()。grep -rn "\\.clone()\\|\\.to_string()\\|\\.to_owned()" core/router-rs/src/ | head -50。`,
  { label: 'adversarial:性能', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
内存安全: 不必要Box<Vec>/Vec<Vec>嵌套、字符串大量重复分配、集合无限增长、泄露文件句柄/连接、#[derive(Clone)]大结构体不必要克隆。`,
  { label: 'adversarial:内存', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
边界条件: 空skill目录路由优雅处理、超长查询长度限制、Unicode/CJK输入正确处理、空JSON默认值、并发skill注册竞态、权限不足有意义错误消息。`,
  { label: 'adversarial:边界条件', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
Hook绕过: hook_common/outbound.rs保护列表遗漏、web_fetch_guard.rs重定向绕过、路径编码绕过(%2e%2e%2f)、大小写不一致绕过。`,
  { label: 'adversarial:Hook绕过', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
依赖安全审计: cargo audit 2>&1(如可用)。Cargo.lock已知漏洞、Python依赖漏洞、不必要依赖、版本更新需求。`,
  { label: 'adversarial:依赖', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
状态损坏恢复: 手动损坏EVIDENCE_INDEX.json、删除SESSION_SUMMARY、写入无效JSON到GOAL_STATE。框架是否能优雅恢复？`,
  { label: 'adversarial:状态恢复', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
信息泄露: 检查错误消息是否泄露敏感信息(路径、token、密钥)。检查日志输出是否包含敏感数据。检查配置文件中的secret。`,
  { label: 'adversarial:信息泄露', phase: 'Phase9-AdversarialTest', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 10: Verify (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase10-Verify')

await agent(
  `面向用户的可见输出使用简体中文。
验证安全发现: 对Phase1和Phase9安全问题逐一验证。尝试构造PoC复现。判定confirmed/false_positive/needs_investigation。`,
  { label: 'verify:安全', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证死代码: 对Phase4发现逐一验证。cargo build确认编译器也认为死代码。grep检查宏/反射使用。检查#[cfg]条件。`,
  { label: 'verify:死代码', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证过期设计: 对Phase5发现逐一验证。读取原始文档和代码确认不一致存在。检查是否描述历史版本。`,
  { label: 'verify:过期设计', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证宿主特化: 对Phase7发现逐一验证。检查"宿主特化"内容是否确实特化(可能所有宿主适用)。检查是否可抽象为共用。`,
  { label: 'verify:宿主特化', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证测试覆盖: 对Phase6发现逐一验证。标记"无测试"模块是否真无测试(可能在其他文件)。cargo test跳过统计。集成测试覆盖单元缺失场景？`,
  { label: 'verify:测试覆盖', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证性能问题: 对Phase9性能发现逐一验证。热点路径复杂度分析。clone()是否真的不必要。字符串分配优化可行性。实际影响评估。`,
  { label: 'verify:性能', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证文档不一致: 对Phase3发现逐一验证。读取原始文档和代码确认。检查是否描述历史版本(有意)。检查是否因最近代码变更。`,
  { label: 'verify:文档', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
验证harness问题: 对Phase2发现逐一验证。运行相关测试确认行为。读取源码确认接口设计问题。检查是否设计意图而非bug。`,
  { label: 'verify:Harness', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
当前构建状态验证: cargo build 2>&1, cargo test 2>&1, cargo clippy 2>&1。记录当前实际构建状态(通过/失败/警告数)。`,
  { label: 'verify:构建状态', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
依赖安全验证: 对Phase9依赖发现逐一验证。CVE实际影响评估(是否使用受影响代码路径)。依赖可安全更新？替代库？`,
  { label: 'verify:依赖安全', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
跨维度交叉验证: 安全修复commit是否修复了安全扫描问题。死代码与过期设计重叠？测试覆盖缺口与对抗测试相关？文档不一致与host-specific相关？`,
  { label: 'verify:交叉验证', phase: 'Phase10-Verify', schema: VERDICT }
)

await agent(
  `面向用户的可见输出使用简体中文。
运维文档验证: 对Phase8发现逐一验证。文档缺失是否确实影响运维。优化建议是否可行。优先级评估。`,
  { label: 'verify:运维文档', phase: 'Phase10-Verify', schema: VERDICT }
)

// ═══════════════════════════════════════════════════════
// Phase 11: Fix Planning (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase11-Plan')

await agent(
  `面向用户的可见输出使用简体中文。
规划安全修复P0: 基于Phase10验证确认的安全问题。影响文件、修复方案、优先级、预估工作量。`,
  { label: 'plan:安全修复', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划死代码清除: 分批清除(先低风险后高风险)、每批验证命令、清除顺序和依赖关系。`,
  { label: 'plan:死代码', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划文档更新: 需更新文档列表、更新优先级(先影响最大)、文档间依赖关系。`,
  { label: 'plan:文档更新', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划过期设计清理: 删除过期文档、更新设计文档、同步代码变更。`,
  { label: 'plan:过期设计', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划测试补充: 最需补充测试模块(按风险排序)、测试策略(单元/集成/契约)、预估工作量。`,
  { label: 'plan:测试补充', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划宿主特化修复: 从共用文件移除特化、移到宿主特定文件、抽象为共用接口。`,
  { label: 'plan:宿主特化', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划运维文档优化: 需新增文档、优化现有文档、多端同步方案、新宿主接入指南。`,
  { label: 'plan:运维文档', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划性能优化: 最需优化热点(按影响排序)、优化策略、benchmark验证需求。`,
  { label: 'plan:性能优化', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划构建垃圾清理: 需清理文件列表、.gitignore补充规则、清理顺序。`,
  { label: 'plan:清理', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
规划依赖更新: 需更新依赖列表、更新顺序(先patch后major)、Cargo.lock更新。`,
  { label: 'plan:依赖更新', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
综合优先级排序: P0安全漏洞/构建失败/数据丢失。P1重要功能/文档错误。P2死代码/过期设计/测试缺口。P3性能/文档打磨/代码风格。输出全局排序修复清单。`,
  { label: 'plan:优先级排序', phase: 'Phase11-Plan', schema: PLAN }
)

await agent(
  `面向用户的可见输出使用简体中文。
制定执行时间线: 基于优先级排序，将修复分配到执行批次。每批次预估耗时、可并行项、依赖关系。`,
  { label: 'plan:时间线', phase: 'Phase11-Plan', schema: PLAN }
)

// ═══════════════════════════════════════════════════════
// Phase 12: Fix (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase12-Fix')

await agent(
  `面向用户的可见输出使用简体中文。
执行P0安全修复: 修复路径遍历防护缺口、hook保护列表遗漏、输入验证缺口。读取目标文件→应用修复→cargo check→运行相关测试。记录修改文件。`,
  { label: 'fix:P0安全', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
修复构建错误: cargo build 2>&1获取错误→修复→cargo build验证→cargo clippy修严重警告。`,
  { label: 'fix:构建错误', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
清除死代码批次1(低风险): 删除未使用import、未使用函数(编译器确认)、未使用变量。每个删除后cargo check。`,
  { label: 'fix:死代码B1', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
清除死代码批次2(需确认): 删除未使用模块(确保无pub use)、废弃feature flag、过期cfg分支。每个删除后cargo test。`,
  { label: 'fix:死代码B2', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
更新文档批次1: AGENTS.md系列(引用过期)、README.md(架构过期)、断裂文档链接、过期命令示例。`,
  { label: 'fix:文档B1', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
更新文档批次2: docs/目录过期设计文档、与代码不一致描述、MCP工具文档、harness文档。`,
  { label: 'fix:文档B2', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
修复宿主特化: 从共用文件移除宿主特化内容→移到正确宿主文件→修复投影漂移→确保共用文件通用。`,
  { label: 'fix:宿主特化', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
优化运维文档: 创建/更新故障排除、多端同步文档、新宿主接入指南、日常运维checklist。`,
  { label: 'fix:运维文档', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
补充关键测试: closeout_enforcement advisory测试、路径遍历防护回归测试、hook保护列表完整性测试。运行新测试确认通过。`,
  { label: 'fix:测试补充', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
更新依赖: cargo update更新patch版本→cargo build验证→cargo test验证。`,
  { label: 'fix:依赖更新', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
修复性能问题: 移除不必要clone()、优化不必要字符串分配、用引用替代所有权转移(安全时)。cargo test验证行为不变。`,
  { label: 'fix:性能优化', phase: 'Phase12-Fix', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
改善错误处理: 库代码unwrap()替换为?/proper error、改善expect()消息、确保Result正确处理。仅修确定安全的替换。cargo check验证。`,
  { label: 'fix:错误处理', phase: 'Phase12-Fix', mode: 'auto' }
)

// ═══════════════════════════════════════════════════════
// Phase 13: Cleanup (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase13-Cleanup')

await agent(
  `面向用户的可见输出使用简体中文。
清理二进制产物: find . -name "*.pyc" -delete; find . -type d -name "__pycache__" -exec rm -rf {} +; find . -name ".DS_Store" -delete; rm -f .test-output.log。列出清理的文件。`,
  { label: 'cleanup:二进制', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
更新.gitignore: 确保包含*.pyc, __pycache__, .DS_Store, .venv/, .ruff_cache/, *.log, .test-output.log。检查遗漏的忽略规则。`,
  { label: 'cleanup:gitignore', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
Cargo构建产物: du -sh target/。检查是否有不必要的Cargo.lock变更。Cargo.toml依赖优化建议。报告但不执行cargo clean。`,
  { label: 'cleanup:Cargo', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
过期分支: git branch -a。识别已合并本地分支、远程过期分支。dependabot分支是否需合并。列出建议删除的分支但不执行。`,
  { label: 'cleanup:过期分支', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
空目录: find . -type d -empty -not -path "./.git/*"。检查空skill目录、空测试目录。`,
  { label: 'cleanup:空目录', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
重复/临时文件: 检查相同内容文件、.bak/.old/.orig备份、.tmp/.swp临时文件、core dump。`,
  { label: 'cleanup:重复文件', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
archive-cold审计: skills/.archive-cold/内容检查。应完全删除的归档？归档文件被其他地方引用？可安全清理？`,
  { label: 'cleanup:归档', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
构建脚本审计: Justfile过期recipe？scripts/可合并脚本？功能重叠？临时文件清理？`,
  { label: 'cleanup:构建脚本', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
JSON工件清理: artifacts/JSON文件检查、可归档旧产物、格式正确性、敏感信息。`,
  { label: 'cleanup:JSON工件', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
缓存清理: .ruff_cache/大小内容、.venv/重建需求、其他缓存目录。`,
  { label: 'cleanup:缓存', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
.githooks审计: .githooks/内容检查、是否有过期hook、hook脚本正确性。`,
  { label: 'cleanup:githooks', phase: 'Phase13-Cleanup', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
清理后最终检查: cargo build验证编译、cargo test验证测试、git status检查未跟踪文件、确认工作目录干净。`,
  { label: 'cleanup:最终检查', phase: 'Phase13-Cleanup', mode: 'auto' }
)

// ═══════════════════════════════════════════════════════
// Phase 14: Review Diff (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase14-ReviewDiff')

await agent(
  `面向用户的可见输出使用简体中文。
审查安全修复diff: git diff检查安全变更。路径遍历防护正确？hook保护完整？输入验证充分？无新安全风险？`,
  { label: 'review:安全diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查死代码清除diff: 确认无误删、无编译错误、测试通过、遗漏清理。`,
  { label: 'review:死代码diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查文档更新diff: 内容准确？格式正确？链接有效？typo？`,
  { label: 'review:文档diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查宿主特化修复diff: 共用文件无宿主特化？宿主文件含正确特化？配置一致？功能不破坏？`,
  { label: 'review:宿主diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查测试补充diff: 新测试覆盖正确场景？独立可运行？命名清晰？无副作用？`,
  { label: 'review:测试diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查性能优化diff: clone()移除安全？字符串优化正确？无新性能问题？行为不变？`,
  { label: 'review:性能diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查整体diff: git diff --stat统计变更规模。总文件数和行数、变更最大文件、意外大规模变更、符合预期？`,
  { label: 'review:整体diff', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查一致性: 风格一致？遗漏配套变更？接口契约不破坏？Cargo.lock更新？`,
  { label: 'review:一致性', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
审查破坏性变更: pub API变更？配置格式变更？CLI参数变更？MCP schema变更？影响和迁移路径评估。`,
  { label: 'review:破坏性', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
最终构建验证: cargo build 2>&1, cargo test 2>&1, cargo clippy -- -D warnings 2>&1。确认全部通过无警告。`,
  { label: 'review:最终构建', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
安全回归检查: 对比修复前后，确认安全修复未引入新漏洞。特别检查路径处理、输入验证、权限检查。`,
  { label: 'review:安全回归', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

await agent(
  `面向用户的可见输出使用简体中文。
文档准确性最终检查: 快速扫描所有修改过的.md文件，确认内容技术上准确、无typo、格式正确。`,
  { label: 'review:文档终检', phase: 'Phase14-ReviewDiff', schema: FINDING }
)

// ═══════════════════════════════════════════════════════
// Phase 15: Merge (10 agents)
// ═══════════════════════════════════════════════════════
phase('Phase15-Merge')

await agent(
  `面向用户的可见输出使用简体中文。
合并状态评估: git status, git branch, git log main..HEAD, git log HEAD..main。评估合并策略。`,
  { label: 'merge:状态评估', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
lifecycle-simplify分支评估: git log main..lifecycle-simplify --oneline, git diff main..lifecycle-simplify --stat。是否需合并？冲突？`,
  { label: 'merge:lifecycle分支', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
远程分支评估: git branch -r列出。每个分支(cursor/paper-adversarial-skills, dependabot/*, feat/deep-audit-and-retire-ts-browser-mcp): 已合并？需合并？可删除？`,
  { label: 'merge:远程分支', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
冲突检查: git merge --no-commit --no-ff lifecycle-simplify测试。列出冲突文件。git merge --abort取消。评估解决策略。`,
  { label: 'merge:冲突检查', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
合并计划: 合并顺序、哪些分支合并、哪些删除、合并策略(merge vs rebase)。`,
  { label: 'merge:合并计划', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
暂存区准备: git add所有变更。git status确认。检查不该提交文件、遗漏文件。`,
  { label: 'merge:暂存区', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
分支管理策略: 保留lifecycle-simplify？release分支？远程清理？未来命名约定。`,
  { label: 'merge:分支策略', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
合并前检查: cargo build通过、cargo test通过、git diff --check无冲突标记、TODO/FIXME处理。`,
  { label: 'merge:合并前检查', phase: 'Phase15-Merge' }
)

await agent(
  `面向用户的可见输出使用简体中文。
合并执行: 在main上git merge lifecycle-simplify(如无冲突)。验证合并后cargo test通过。`,
  { label: 'merge:执行合并', phase: 'Phase15-Merge', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
合并后验证: cargo build, cargo test, git log --oneline -10。确认最终状态正确。`,
  { label: 'merge:合并后验证', phase: 'Phase15-Merge' }
)

// ═══════════════════════════════════════════════════════
// Phase 16: Commit & Push (12 agents)
// ═══════════════════════════════════════════════════════
phase('Phase16-CommitPush')

await agent(
  `面向用户的可见输出使用简体中文。
分批commit计划: 基于所有变更分批: 安全修复fix(security)、死代码chore:deadcode、文档docs、宿主fix(host)、测试test、性能perf、运维docs(ops)。每批commit message和文件列表。`,
  { label: 'commit:计划', phase: 'Phase16-CommitPush' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次1-安全修复: git add安全修复文件 → git commit -m "fix(security): ..." → git log -1确认。只commit安全相关变更。`,
  { label: 'commit:安全批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次2-死代码: git add死代码清除文件 → git commit -m "chore: 全量死代码清理" → git log -1确认。`,
  { label: 'commit:死代码批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次3-文档: git add文档文件 → git commit -m "docs: 全量文档审计更新" → git log -1确认。`,
  { label: 'commit:文档批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次4-宿主特化: git add宿主特化文件 → git commit -m "fix(host): 共用资产去宿主特化" → git log -1确认。`,
  { label: 'commit:宿主批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次5-测试+性能: git add测试和性能文件 → git commit -m "test+perf: 测试补充和性能优化" → git log -1确认。`,
  { label: 'commit:测试性能批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次6-运维文档: git add运维文档 → git commit -m "docs(ops): 运维文档全面优化" → git log -1确认。`,
  { label: 'commit:运维批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行commit批次7-配置清理: git add配置/gitignore变更 → git commit -m "chore: 配置清理和gitignore更新" → git log -1确认。`,
  { label: 'commit:配置批次', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
最终commit检查: git status干净？git log --oneline -10查看。cargo test确认通过。所有变更已提交。`,
  { label: 'commit:最终检查', phase: 'Phase16-CommitPush' }
)

await agent(
  `面向用户的可见输出使用简体中文。
push准备: git log --oneline origin/main..HEAD查看待push。确认commit message格式、无敏感信息、Cargo.lock状态。`,
  { label: 'push:准备', phase: 'Phase16-CommitPush' }
)

await agent(
  `面向用户的可见输出使用简体中文。
执行push: git push origin main。确认push成功。git log --oneline -5确认远程状态。`,
  { label: 'push:执行', phase: 'Phase16-CommitPush', mode: 'auto' }
)

await agent(
  `面向用户的可见输出使用简体中文。
push后清理: git fetch --prune清理过期远程跟踪。确认最终状态干净。输出全量审计总结报告。`,
  { label: 'push:后清理', phase: 'Phase16-CommitPush', mode: 'auto' }
)

// ═══════════════════════════════════════════════════════
// 最终汇总
// ═══════════════════════════════════════════════════════
log('=== 16维度×200+agent全量收尾审计完成 ===')
return { status: 'complete', phases: 16, totalAgents: '200+' }
