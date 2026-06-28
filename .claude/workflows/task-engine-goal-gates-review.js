export const meta = {
  name: 'task-engine-goal-gates-review',
  description: '深度对抗性 review：Task Engine + Goal Management + Exit Gates',
  phases: [
    { title: 'Task Engine', detail: '对抗性 review task_tools.rs, task_state.rs, transition_validation.rs' },
    { title: 'Goal Mgmt', detail: '对抗性 review goal_ops.rs, goal-engine/' },
    { title: 'Exit Gates', detail: '对抗性 review QGEntry + Closeout' },
    { title: 'Synthesis', detail: '去重/汇总/标记优先级' },
  ],
}

// Shared schema for structured findings
const FINDING_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'e.g. TASK-001, GOAL-001, GATE-001' },
          severity: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
          file: { type: 'string', description: 'file path with line range' },
          title: { type: 'string', maxLength: 120 },
          description: { type: 'string', description: 'detailed explanation of the issue' },
          reproduction: { type: 'string', description: 'conditions under which this bug manifests' },
          impact: { type: 'string', enum: ['crash', 'data_loss', 'incorrect_behavior', 'security', 'performance', 'maintainability', 'design_flaw'] },
          recommendation: { type: 'string' },
        },
        required: ['id', 'severity', 'file', 'title', 'description', 'impact', 'recommendation'],
      },
    },
  },
  required: ['findings'],
}

const STATS_SCHEMA = {
  type: 'object',
  properties: {
    files_read: { type: 'array', items: { type: 'string' } },
    total_lines_scanned: { type: 'number' },
    coverage_notes: { type: 'string' },
  },
  required: ['files_read', 'total_lines_scanned'],
}

// ──────────────────────────────────────────────
// Phase 1+2+3: three parallel agents
// ──────────────────────────────────────────────

phase('Task Engine')
phase('Goal Mgmt')
phase('Exit Gates')

const [taskEngine, goalMgmt, exitGates] = await pipeline(
  ['task-engine', 'goal-mgmt', 'exit-gates'],
  // Stage 1: spawn the three review agents in parallel
  (area) => {
    if (area === 'task-engine') {
      return agent(`你是一位对抗性代码审查专家。请对 Task Engine 子系统进行深度对抗性 review。

## 范围文件

必读文件（按优先级）：
1. /Users/joe/Developer/skill/core/host-projection/src/hosts/mcp_stdio_harness/task_tools.rs — task_create/task_complete/task_focus/task_chain_advance 实现
2. /Users/joe/Developer/skill/core/core-state/src/transition_validation.rs — validate_transition, TaskTransition 枚举
3. /Users/joe/Developer/skill/core/core-state/src/task_state.rs — resolve_task_view, hydration, 指针管理
4. /Users/joe/Developer/skill/core/core-state/src/task_ledger.rs — LedgerTransaction, append, checkpoint
5. /Users/joe/Developer/skill/core/core-state/src/state_manager/pointer_ops.rs — 指针读写, sync_task_pointers_after_goal_drive
6. /Users/joe/Developer/skill/core/core-state-utils/src/task_write_lock.rs — apply_task_ledger_mutation, 文件锁
7. /Users/joe/Developer/skill/core/runtime-core/src/framework_runtime/tool_handlers/task_handler.rs — task_complete_dispatch 等

## 检查维度

1. **幂等性**: task_create 是否真正幂等？重复调用会有什么副作用？
2. **指针一致性**: active/focus 指针在所有操作（create/complete/focus/chain_advance）后是否一致？是否存在导致指针漂移的路径？
3. **锁覆盖**: apply_task_ledger_mutation 覆盖了哪些操作？是否有写操作遗漏了锁？
4. **错误路径**: 路径遍历保护、空值处理、并发写入、文件不存在、权限错误
5. **task_chain_advance 边界**: 空链、单任务、末尾越界、loop goal 跳过逻辑是否正确
6. **账本**: append 失败、checkpoint 不一致、compaction 时机
7. **transition_validation**: Complete 的证据检查逻辑是否充分？auto-pass 规则是否有风险？
8. **对抗性角度**: "如何破坏这个系统"？同时调用 create/complete？篡改指针文件？带路径遍历字符？空 task_id？

## 输出要求

返回结构化 findings 列表。每个 finding 必须包含文件路径（或行号范围）、严重度 P0~P3、标题、详细描述、重现条件（如有）、影响分类、修复建议。

面向用户的可见输出使用简体中文。`, {label: 'Task Engine review', phase: 'Task Engine', schema: FINDING_SCHEMA})
    }
    if (area === 'goal-mgmt') {
      return agent(`你是一位对抗性代码审查专家。请对 Goal Management 子系统进行深度对抗性 review。

## 范围文件

必读文件（按优先级）：
1. /Users/joe/Developer/skill/core/core-state/src/state_manager/goal_ops.rs — framework_goal_drive 主状态机（~2000行） — 请完整阅读
2. /Users/joe/Developer/skill/core/runtime-core/src/framework_runtime/tool_handlers/goal_handler.rs — goal_state_manage_dispatch
3. /Users/joe/Developer/skill/core/core-state/src/state_manager/mod.rs — read_goal_state, goal_state_path_for_task
4. /Users/joe/Developer/skill/core/goal-engine/src/types.rs — LoopPhase, LoopRunState, LoopAction
5. /Users/joe/Developer/skill/core/goal-engine/src/state.rs — transition_phase, state read/write
6. /Users/joe/Developer/skill/core/goal-engine/src/runner.rs — loop 运行器
7. /Users/joe/Developer/skill/core/goal-engine/src/closeout.rs — verify_closeout_value, build_aggregate

## 检查维度

1. **状态机转换完整性**: goal_ops 中每个 operation（start/complete/checkpoint/pause/resume/block/clear/amend/continue_review/retry）的输入/输出状态是否正确？是否有遗漏的状态转换？
2. **validate_drive_contract**: drive_until_done 的契约检查是否充分？>=1 non_goals + >=2 done_when + >=1 validation_commands — 这个配置是否允许循环目标的最基本表达？
3. **complete 语义**: v10 中 complete=iteration_complete（非 terminate）。无终止路径是设计意图还是遗漏？goal 是否可以永远处于 running 状态？
4. **QG auto-trigger**: goal_ops.rs 在 complete 路径上触发 QGEntry（hooks.evaluate_quality_gate）。如果 hooks 不存在？返回错误？返回假通过？是否有 fail-open 风险？
5. **continue_review / retry**: 仅从 review_pending 状态可用。如果 goal 在其他状态调用会怎样？是否保护了所有路径？
6. **amend 安全性**: 哪些字段可以被 amend？有无副作用？是否存在 amend 导致状态机不一致？
7. **session 陈旧性**: session_id stale 检测是否在所有 read/write 路径一致？陈旧 goal 的 amend 保护是否完整？
8. **goal-engine vs core-state 不一致**: 两个 crate 维护独立的 goal 状态（LoopRunState vs GOAL_STATE.json）。是否存在分歧的可能？
9. **对抗性角度**: 连续 complete 会怎样？checkpoint 时删除 GOAL_STATE.json？drive_until_done 但所有 done_when 为空？complete 时 QG 挂死？

## 输出要求

返回结构化 findings 列表。每个 finding 必须包含文件路径（或行号范围）、严重度 P0~P3、标题、详细描述、影响分类、修复建议。

面向用户的可见输出使用简体中文。`, {label: 'Goal Mgmt review', phase: 'Goal Mgmt', schema: FINDING_SCHEMA})
    }
    if (area === 'exit-gates') {
      return agent(`你是一位对抗性代码审查专家。请对两个退出 Gate 进行深度对抗性 review。

## 范围文件

必读文件（按优先级）：
1. /Users/joe/Developer/skill/core/runtime-core/src/qg_entry.rs — trigger(), evaluate_quality_gate_hook — 两阶段出口门
2. /Users/joe/Developer/skill/core/runtime-core/src/qg_route.rs — init_qg_route, evaluate_qg_route
3. /Users/joe/Developer/skill/core/core-state/src/closeout_validation.rs — R1-R8 规则, evaluate_closeout_record_value 及其 _with_context 变体
4. /Users/joe/Developer/skill/core/runtime-core/src/framework_runtime/tool_handlers/closeout_handler.rs — closeout_record_write_dispatch, closeout_gate_evaluate, evaluate_closeout_gate_hook
5. /Users/joe/Developer/skill/core/framework-extra/src/closeout.rs — closeout_stop_followup_for_completion_text, evaluate_closeout_record_file_for_task
6. /Users/joe/Developer/skill/core/goal-engine/src/closeout.rs — verify_closeout_value, build_aggregate
7. /Users/joe/Developer/skill/core/runtime-core/src/framework_runtime/tool_handlers/goal_handler.rs — 查看 goal complete 如何触发 QGEntry

## 检查维度

### Gate 1: QGEntry (两阶段门)

1. **Stage 1 反欺诈门**: task_evidence_artifacts_summary_for_task 的逻辑 —— 何时 pass/block？无证据文件时 auto-pass 是否合理（"empty task list = no fraud possible"）？如果证据文件存在但所有 exit_code != 0 且 success=false —— 这是正确的 fraud 检测吗？
2. **Stage 2 质门**: scene 分发逻辑，CheckerRegistry 初始化失败的风险（fail-closed vs fail-open）
3. **Stage 1 ↔ Stage 2 关系**: EvidenceChecker 在 Stage 2 的 advisory findings 是否真正被消费？Stage 1 和 EvidenceChecker 是否重复检查同一件事？
4. **trigger() 调用的五个入口**: 分别在哪些路径上触发？路径之间是否有行为差异？
5. **hook fail-open**: 如果 evaluate_quality_gate 返回 Err，caller 的行为（goal_ops 继续迭代 vs runner 降级 aggregate）

### Gate 2: Closeout

6. **R1-R8 规则完整性**: 每个规则的触发条件是否清晰？是否有 false positive（规则误触发）？是否有遗漏规则？
7. **closeout_record_write 的设计**: 先写文件再评估 —— 文件已落盘但评估失败，没有回滚。这是设计意图吗？
8. **closeout_gate_evaluate 的 advisory-only**: 它检查 5 个条件但从来不会 block。是否有场景应该 block 但没 block？
9. **closeout_stop_followup**: CI 门控（ROUTER_RS_CLOSEOUT_ENFORCEMENT）—— 本地此功能被禁用意味着 closeout 在开发中完全不执行？
10. **R8 的 context-aware 限制**: claimed_passed_without_evidence_index_rows 仅在 _with_context 变体中存在，基础 evaluate 没包括此规则

### Wire Crossings

11. **shared evidence**: QGEntry 和 closeout gate 都调用 task_evidence_artifacts_summary_for_task，但对相同证据做出不同的决定（QGEntry blocking vs closeout advisory）
12. **runner.rs 中的顺序评估**: QG 先 fire，closeout 后 fire（仅在 QG pass 后）。QG block 时 closeout 完全跳过
13. **自证证据处理**: closeout_gate_evaluate 会检查 self-attested 并 warn，但 QGEntry 不区分。两个 gate 对自证证据的处理不一致

## 输出要求

返回结构化 findings 列表。每个 finding 必须包含文件路径（或行号范围）、严重度 P0~P3、标题、详细描述、影响分类、修复建议。

面向用户的可见输出使用简体中文。`, {label: 'Exit Gates review', phase: 'Exit Gates', schema: FINDING_SCHEMA})
    }
  },
)

// ──────────────────────────────────────────────
// Phase 4: Synthesis — dedup & prioritize
// ──────────────────────────────────────────────

phase('Synthesis')

const allFindings = [
  ...(taskEngine?.findings || []),
  ...(goalMgmt?.findings || []),
  ...(exitGates?.findings || []),
]

const report = await agent(`你是一位深度代码审查报告合成专家。以下是三个独立 agent 对同一代码库不同子系统进行的对抗性 review 发现清单。

## 发现去重与汇总要求

1. **去重**: 如果多个 agent 报告同一问题，只保留一份并标记"跨域发现"
2. **优先级重排**: 按 P0 > P1 > P2 > P3 顺序排列，同优先级内按 impact 排序（crash > data_loss > security > incorrect_behavior > performance > design_flaw > maintainability）
3. **分类汇总**: 添加分类标签（task-engine / goal-mgmt / exit-gates / cross-cutting）
4. **统计**: P0/P1/P2/P3 各多少，总发现数
5. **Top-3 推荐**: 最重要的三个修复建议

## 原始发现

### Task Engine Findings:
${JSON.stringify(taskEngine?.findings || [], null, 2)}

### Goal Management Findings:
${JSON.stringify(goalMgmt?.findings || [], null, 2)}

### Exit Gates Findings:
${JSON.stringify(exitGates?.findings || [], null, 2)}

## 输出格式

返回 markdown 格式的报告，结构如下：
1. 执行摘要（发现总数、严重度分布、整体评估）
2. 发现详情（按 P0→P3 逐项列出，每项含 file:line、标题、描述、影响、修复建议）
3. 分类统计
4. Top-3 优先修复建议

面向用户的可见输出使用简体中文。`, {
  label: 'Synthesis',
  phase: 'Synthesis',
  schema: {
    type: 'object',
    properties: {
      report: { type: 'string', description: '完整的 markdown 审查报告' },
      stats: {
        type: 'object',
        properties: {
          total_findings: { type: 'number' },
          p0: { type: 'number' },
          p1: { type: 'number' },
          p2: { type: 'number' },
          p3: { type: 'number' },
          categories: { type: 'object' },
        },
        required: ['total_findings', 'p0', 'p1', 'p2', 'p3', 'categories'],
      },
    },
    required: ['report', 'stats'],
  },
})

// Write the final report to disk
const finalReport = report?.report || '# Report synthesis failed\n\nOne or more review agents returned no findings.'
const stats = report?.stats || { total_findings: 0, p0: 0, p1: 0, p2: 0, p3: 0, categories: {} }

log(`报告完成：共 ${stats.total_findings} 个发现（P0:${stats.p0} P1:${stats.p1} P2:${stats.p2} P3:${stats.p3}）`)

return { report: finalReport, stats }
