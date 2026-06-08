import { agent, parallel, pipeline, phase, log } from "workflow"
export const meta = {
  name: 'hook-route-deep-audit',
  description: 'Deep audit hook/routing files, fix issues, clean artifacts, commit and push',
  phases: [
    { title: 'Audit', detail: 'multi-agent deep scan of hook/routing files' },
    { title: 'Verify', detail: 'adversarial verify each finding' },
    { title: 'Plan', detail: 'prioritized fix plan' },
    { title: 'Fix', detail: 'implement fixes' },
    { title: 'Clean', detail: 'remove stale binaries and build artifacts' },
    { title: 'Docs', detail: 'update docs, comments, and inline annotations' },
    { title: 'Review', detail: 'final review of all changes' },
    { title: 'Commit', detail: 'batched commits and push' },
  ],
};

// ── Phase 1: Multi-agent deep audit ──
phase('Audit')

const AUDIT_TARGETS = [
  {
    key: 'claude-hooks',
    label: 'claude_hooks.rs',
    prompt:
      '面向用户的可见输出使用简体中文。\n\n' +
      '深度审计 /Users/joe/Developer/skill/core/router-rs/src/hosts/claude_hooks.rs。\n\n' +
      '重点检查：\n' +
      '1. 上一轮改动后的代码一致性：CROSS_HOST_SURFACES/RETIRED_SURFACES/is_cross_host_or_retired_surface/is_framework_source_path/FRAMEWORK_SOURCE_PREFIXES 的引用是否完整\n' +
      '2. 是否有残留的旧名称引用（RETIRED_SURFACE_PATHS、is_retired_surface）\n' +
      '3. run_pre_tool_use 中新增的 warn 逻辑是否正确：AGENTS_CLAUDE.md 的比较是否应该用相对路径匹配而非精确字符串\n' +
      '4. PostToolUse 中 is_framework_source_path 的调用是否覆盖了所有需要提醒的场景\n' +
      '5. Stop 门禁中 touched_framework 标志是否仍然正确触发 framework_tested 检查\n' +
      '6. 测试覆盖：是否有缺失的测试用例（如 warn 返回值的结构验证）\n' +
      '7. 死代码：是否有未使用的函数/常量/导入\n' +
      '8. 文档注释是否与实际行为一致\n\n' +
      '用 bash grep/sed 搜索代码，给出每个发现的文件路径、行号、具体问题和建议修复。',
  },
  {
    key: 'hook-policy',
    label: 'hook_policy.rs',
    prompt:
      '面向用户的可见输出使用简体中文。\n\n' +
      '深度审计 /Users/joe/Developer/skill/core/router-rs/src/hook_policy.rs。\n\n' +
      '重点检查：\n' +
      '1. CODEX_PROTECTED_GENERATED_PATHS 从 10 改为 9 后，数组大小是否正确\n' +
      '2. classify_protected_path 中对 AGENTS_CLAUDE.md 的处理是否需要同步更新\n' +
      '3. 测试 protected_paths_cover_retired_and_codex_surfaces 是否需要更新（不再期望 AGENTS_CLAUDE.md 被保护）\n' +
      '4. save_optimize_guard_respects_protected_paths 测试是否受影响\n' +
      '5. hook_policy 与 claude_hooks 之间的保护列表一致性\n' +
      '6. 是否有死代码或过期注释\n\n' +
      '用 bash grep 搜索，给出具体行号和修复建议。',
  },
  {
    key: 'env-flags',
    label: 'router_env_flags.rs + hook 脚本',
    prompt:
      '面向用户的可见输出使用简体中文。\n\n' +
      '审计以下文件的一致性：\n' +
      '1. /Users/joe/Developer/skill/core/router-rs/src/router_env_flags.rs — 检查新增的 router_rs_skip_pre_tool_use_guard 函数是否符合模块风格（注释格式、命名规范）\n' +
      '2. /Users/joe/Developer/skill/configs/framework/claude-router-rs-hook.sh — hook 启动脚本的路径查找逻辑是否有优化空间\n' +
      '3. /Users/joe/Developer/skill/.claude/router-rs-hook.env — 环境变量设置是否合理\n' +
      '4. /Users/joe/Developer/skill/.claude/settings.json — hooks 配置是否与 claude-router-rs-hook.sh 的事件匹配\n' +
      '5. /Users/joe/Developer/skill/.cargo/config.toml — target-dir 设置导致的问题（二进制不在 hook 脚本搜索路径中）\n\n' +
      '重点：.cargo/config.toml 的 target-dir=/tmp/skill-cargo-target 与 hook 脚本优先搜索 core/router-rs/target/ 之间的不一致，这是一个运维隐患。',
  },
  {
    key: 'cross-host-sync',
    label: '跨宿主保护对称性',
    prompt:
      '面向用户的可见输出使用简体中文。\n\n' +
      '检查跨宿主保护列表的对称性：\n' +
      '1. /Users/joe/Developer/skill/core/router-rs/src/hosts/codex_hooks/mod.rs — PROTECTED_GENERATED_PATHS 是否包含 AGENTS.md 和 AGENTS_CLAUDE.md\n' +
      '2. /Users/joe/Developer/skill/core/router-rs/src/hosts/cursor_hooks/ — Cursor 侧是否有独立的路径保护\n' +
      '3. claude_hooks.rs 的 GENERATED_ENTRYPOINT_PATHS_CLAUDE 现在只有 .claude/CLAUDE.md，而 codex_hooks 的 PROTECTED_GENERATED_PATHS 可能仍有 AGENTS.md — 这种不对称是否合理\n' +
      '4. hook_policy.rs 的 CODEX_PROTECTED_GENERATED_PATHS 和 codex_hooks 的 PROTECTED_GENERATED_PATHS 是否一致\n\n' +
      '给出对称性分析和建议。',
  },
]

const audits = await parallel(AUDIT_TARGETS.map(t => () =>
  agent(
    t.prompt,
    { label: `audit:${t.key}`, phase: 'Audit' }
  )
))

// ── Phase 2: Adversarial verify each finding ──
phase('Verify')

const allFindings = audits.join('\n\n---\n\n')

const verified = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '你是一个严格的验证员。以下是 4 个审计 agent 的发现。逐条验证每个发现是否真实。\n\n' +
  '验证方法：\n' +
  '1. 对每个发现，用 grep/cat 读取实际源码确认\n' +
  '2. 检查发现中的行号是否准确\n' +
  '3. 确认问题是否确实存在（不是误报）\n' +
  '4. 评估严重程度：P0（编译失败/安全漏洞）、P1（功能缺陷）、P2（代码质量）、P3（文档/注释）\n\n' +
  '输出格式：对每个发现给出 [确认/误报/需进一步调查] + 严重程度 + 实际行号。\n\n' +
  '审计结果：\n' + allFindings,
  { label: 'verify:adversarial', phase: 'Verify' }
)

// ── Phase 3: Prioritized fix plan ──
phase('Plan')

const plan = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '基于验证后的发现，制定分批修复计划。\n\n' +
  '验证结果：\n' + verified + '\n\n' +
  '要求：\n' +
  '1. 按 P0→P1→P2→P3 优先级排序\n' +
  '2. 每个修复标注：文件路径、行号、修改内容\n' +
  '3. 分批策略：哪些修复应该在同一个 commit 中\n' +
  '4. 编译垃圾清理策略：哪些旧二进制/artifact 可以安全删除\n' +
  '5. 文档更新清单\n\n' +
  '输出为结构化的修复清单。',
  { label: 'plan:fix-plan', phase: 'Plan' }
)

// ── Phase 4: Implement fixes ──
phase('Fix')

const fixResult = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '执行以下修复计划。逐条修改文件，每改一个文件就编译验证。\n\n' +
  '修复计划：\n' + plan + '\n\n' +
  '执行规则：\n' +
  '1. 先改代码，再编译验证（unset CARGO_TARGET_DIR && cargo build -p router-rs）\n' +
  '2. 每个修改用 python3 脚本精确替换（不要用 sed，容易出错）\n' +
  '3. 编译失败就回退该修改\n' +
  '4. 记录每个实际修改的文件和行号\n\n' +
  'binary 路径注意：.cargo/config.toml 设置了 target-dir=/tmp/skill-cargo-target，编译产物在 /tmp/skill-cargo-target/debug/router-rs，' +
  '需要 cp 到 core/router-rs/target/debug/router-rs 才能被 hook 脚本找到。',
  { label: 'fix:implement', phase: 'Fix' }
)

// ── Phase 5: Clean stale binaries ──
phase('Clean')

const cleanResult = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '清理编译垃圾和旧二进制。执行以下操作：\n\n' +
  '1. 列出 /tmp/skill-cargo-target/ 的大小：du -sh /tmp/skill-cargo-target/\n' +
  '2. 列出 /Users/joe/Developer/skill/core/router-rs/target/ 的大小\n' +
  '3. 列出 /Users/joe/Developer/skill/target/ 的大小\n' +
  '4. 检查 .claude/worktrees/ 下是否有可以清理的旧 worktree\n' +
  '5. 检查是否有其他 stale 的 target/debug 目录\n\n' +
  '只做清理操作，不要删除正在使用的二进制。最终确认清理结果和释放的空间。',
  { label: 'clean:artifacts', phase: 'Clean' }
)

// ── Phase 6: Update docs and comments ──
phase('Docs')

const docsResult = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '更新文档和代码注释，确保与实际行为一致：\n\n' +
  '1. claude_hooks.rs 中的模块文档注释（//! 开头）是否需要更新\n' +
  '2. FRAMEWORK_GUARDED_PREFIXES 和 FRAMEWORK_SOURCE_PREFIXES 的注释是否清晰\n' +
  '3. CROSS_HOST_SURFACES 和 RETIRED_SURFACES 的注释是否解释了区别\n' +
  '4. router_env_flags.rs 中 router_rs_skip_pre_tool_use_guard 的文档是否完整\n' +
  '5. 检查 docs/harness_architecture/ 下是否有需要更新的文档\n' +
  '6. 检查 docs/hosts/ 下是否有过期引用（如 antigravity-cli.md 中对 .agents 的引用）\n\n' +
  '只更新注释和文档，不改逻辑。用 python3 脚本精确替换。',
  { label: 'docs:update', phase: 'Docs' }
)

// ── Phase 7: Final review ──
phase('Review')

const finalReview = await parallel([
  () => agent(
    '面向用户的可见输出使用简体中文。\n\n' +
    '安全审计：检查所有修改是否引入安全漏洞。\n\n' +
    '用 git diff 查看所有未提交的改动，逐文件审查：\n' +
    '1. 是否有路径遍历风险\n' +
    '2. 保护是否被不当移除\n' +
    '3. warn 逻辑是否可能被绕过\n' +
    '4. 新增的常量/函数是否正确导出和使用\n\n' +
    '运行 unset CARGO_TARGET_DIR && cargo test -p router-rs 验证测试。\n' +
    '给出安全通过/阻断的判定。',
    { label: 'review:security', phase: 'Review' }
  ),
  () => agent(
    '面向用户的可见输出使用简体中文。\n\n' +
    '功能验证：确认所有 hook 行为符合预期。\n\n' +
    '用如下命令逐项测试（ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD=0）：\n' +
    'binary: /Users/joe/Developer/skill/core/router-rs/target/debug/router-rs\n' +
    'repo: /Users/joe/Developer/skill\n\n' +
    '测试清单：\n' +
    '1. AGENTS.md -> pass\n' +
    '2. AGENTS_CLAUDE.md -> warn\n' +
    '3. .claude/CLAUDE.md -> deny\n' +
    '4. core/router-rs/src/lib.rs -> pass\n' +
    '5. configs/framework/xxx -> deny\n' +
    '6. skills/SKILL_TIERS.json -> deny\n' +
    '7. skills/SKILL_MAINTENANCE_GUIDE.md -> pass\n' +
    '8. .claude/settings.json -> warn\n' +
    '9. .codex/hooks.json -> deny\n\n' +
    'binary 路径注意：编译产物在 /tmp/skill-cargo-target/debug/router-rs，需要 cp 到 core/router-rs/target/debug/。\n' +
    '给出通过/失败判定。',
    { label: 'review:functional', phase: 'Review' }
  ),
])

// ── Phase 8: Batched commits and push ──
phase('Commit')

const commitResult = await agent(
  '面向用户的可见输出使用简体中文。\n\n' +
  '执行分批 commit 和 push。\n\n' +
  '分批策略：\n' +
  '1. 第一批：hook guard 优化（claude_hooks.rs + hook_policy.rs 的保护列表调整）\n' +
  '2. 第二批：env flags + hook 脚本 + 配置文件更新\n' +
  '3. 第三批：文档和注释更新\n' +
  '4. 第四批：编译垃圾清理（如果有 .gitignore 变更）\n\n' +
  '每个 commit 使用中文 commit message，格式：\n' +
  '类型(scope): 简短描述\n\n' +
  '类型可选：feat/fix/refactor/docs/chore/test\n\n' +
  '最后 git push origin lifecycle-simplify。\n\n' +
  '注意：不要 force push，不要修改 .git 之外的不可追踪文件。',
  { label: 'commit:batch-push', phase: 'Commit' }
)

return {
  audits,
  verified,
  plan,
  fixResult,
  cleanResult,
  docsResult,
  finalReview,
  commitResult,
}
