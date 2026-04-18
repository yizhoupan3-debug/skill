---
name: checklist-fixer
description: |
  Auto-proceeded by agent trigger. Systematic fix-list execution of 
  structured issue queues, audit findings, or implementation plans. 
  Use when: "Auto-proceeded by agent.", "逐项修复", "fix list", 
  or selecting items by number. Follows an existing plan to ensure 
  complete delivery.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 逐项修复
  - fix list
  - 按 checklist 执行
  - 先做 1-3
  - 从 P0 开始
  - 只做第一个
short_description: Execute fix lists and implementation plans with mandatory per-item verification and anti-laziness enforcement.
metadata:
  version: "2.1.0"
  platforms: [codex, antigravity]
  tags:
    - batch-fix
    - checklist
    - issue-execution
    - audit-fix
    - plan-execution
    - priority-queue
    - anti-laziness
    - verification
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
  anti_laziness_fused: true
risk: medium
source: local
---

# Checklist Fixer

This skill owns **checklist-driven fix execution**: take a known list of
problems or a plan and systematically fix / implement them, one item at a
time, with **mandatory verification** after each item.  Anti-laziness
enforcement is **always active** — no item is done until tool output proves it.

## Priority routing rule

If the previous assistant turn contained a plan, implementation plan, numbered
list, or checklist **AND the user's current message signals intent to execute**
(see triggers below), route to this skill **before** general implementation or
debugging skills.

**Broad trigger set (route here when any of these match):**

- Explicit execution words: "执行", "开始", "做", "fix", "implement", "run",
  "落实", "搞定", "干", "帮我做", "按这个来", "都做了", "Auto-proceeded by agent."
- Minimal confirmations after a plan: "好", "好的", "行", "可以", "ok", "yes",
  "approve", "确认", "没问题", "就这样", "就按这个", "同意", "👍"
- Item selection by number: user replies "1", "2", "3", "1和3", "先做2",
  "1-3", "只做第一个" → execute only the selected items
- "先做 X 后做 Y", "从 P0 开始", "按优先级", "逐项", "全部"
- User references a specific artifact: "按 PLAN.md 做", "把 implementation_plan 落实"

Before this skill takes ownership, respect earlier gates:

1. Root cause unknown for any item → `$systematic-debugging` first for that item
2. Problem list comes from PR review comments → `$gh-address-comments`
3. Problem list comes from CI failures → `$gh-fix-ci`
4. Task is structural refactoring without a fix list → `$refactoring`
5. User wants a plan only, not execution → `$checklist-writting`
6. Task is complex enough for delegation → coordinate with `$subagent-delegation`

## When to use

- Previous turn produced a plan/checklist and user signals go-ahead
- User gives a single-digit or range reply selecting items to execute
- User has a numbered list of known problems and wants them fixed
- An upstream skill produced a problem list (audit, review, scan) needing execution
- `$paper-reviewer` already produced a manuscript issue ledger and the user now wants selected items actually fixed
- User provides a `PLAN.md`, `implementation_plan.md`, or audit report with `- [ ]` items
- Fixes are mixed types (bugs, missing validation, dead code, config issues)
- User wants progress tracking with checkpoint/resume capability

## Do not use

- Root cause is unknown and investigation still needed → `$systematic-debugging`
- Task is implementing a feature from a PRD/spec (no existing list) → `$plan-to-code`
- List comes from GitHub PR comments → `$gh-address-comments`
- List comes from failing CI checks → `$gh-fix-ci`
- Purely structural refactoring without a fix list → `$refactoring`
- User only wants a plan/assessment, not execution → `$checklist-writting` or `$architect-review`

## Primary operating principle

This owner should behave like a **queue executor inside the master-control chain**:

1. execute one bounded checklist item at a time
2. keep per-item evidence and verification outside the main thread when possible
3. prefer bounded sidecars for non-blocking subwork when runtime policy permits
4. keep shared continuity artifacts supervisor-only even when checklist items run in parallel
5. if runtime policy blocks spawning, preserve the same checklist split in local-supervisor mode
6. report progress as queue state, not as sprawling process narration

## Main-thread compression contract

The main thread should contain only:

- current item
- queue progress
- blocker or verification result
- reroute decision if needed
- next item

## Runtime-policy adaptation

If runtime policy permits delegation:

- route bounded research, verification, or isolated implementation slices through [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
- require lane-local outputs or delta artifacts; do not let delegated items co-edit global continuity files

If runtime policy does **not** permit spawning:

- keep the same checklist split in local-supervisor mode
- execute items or side-slices sequentially
- keep raw per-item detail in artifacts, notes, or state instead of the main thread
- flush `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, and `.supervisor_state.json` only from the integrating controller step

## Task ownership and boundaries

This skill owns:
- detecting execution intent from minimal confirmation signals
- scoping execution to user-selected items when numbers are given
- parsing and prioritizing fix lists from any structured input
- per-item fix execution with **mandatory verification**
- anti-laziness self-enforcement throughout the session
- progress tracking (checkbox updates, checkpoint files)
- resume from interrupted sessions
- atomic or grouped fix commits when traceability is preserved

This skill does not own:
- discovering problems (that's upstream skills' job)
- root-cause investigation for unclear bugs
- feature implementation from specs
- CI/PR-specific feedback workflows
- restructuring a messy checklist before execution when serial/parallel boundaries, goals, constraints, or acceptance are still unclear → `$checklist-normalizer`

## Finding-driven framework role

This skill is a **Phase-1 planner / executor / verifier anchor** in the shared
finding-driven framework. It consumes structured findings from upstream skills
via the shared schema in [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md),
while remaining backward-compatible with legacy markdown checklists and issue queues.

Supported input forms:
- markdown checklists (`- [ ]`)
- numbered issue lists
- upstream findings ledgers with `finding_id`, `severity_native`
- `implementation_plan.md` proposed-changes sections
- mixed issue sets once an upstream owner has normalized them

When an upstream detector already provides `finding_id`, `severity_native`, or
`verification_method`, preserve those fields instead of flattening them away.

Common downstream sources:
- `$paper-reviewer` issue ledgers for manuscript repair execution
- audit / review / scan outputs that already decided the queue

## Required workflow

1. **Parse & scope**: if user specified items by number/selection, restrict queue to those items only.
2. **Build queue**: prioritize by P0 → P1 → P2 → P3, then dependency order.
3. **Ask once** if scope is genuinely ambiguous; otherwise proceed directly.
4. **Execute & verify each item** — see verification protocol below.
5. **Mark blocked or failed items** explicitly; never hide them.
6. **Finish with completion summary** using the template in `references/execution-workflow.md`.

## Anti-laziness enforcement (ALWAYS ACTIVE)

This skill permanently fuses the anti-laziness overlay. The following rules
are non-negotiable for every item:

- **No unverified completions**: every fix must be confirmed with tool output
  (`stdout`, test results, build logs, lint output) before marking `✅`.
- **No code truncation**: write complete replacements; never use `...` or
  `// remains unchanged` placeholders.
- **No passive finish**: "should work now" is forbidden. Prove it with evidence.
- **No context-begging**: search the workspace with tools before asking the user.
- **No wheel-spinning**: if the same approach fails twice, pivot immediately —
  change strategy, not just parameters.
- **False-convergence scan**: after all items done, run a broad check
  (`grep`, tests, build) to catch anything missed.

If any laziness pattern is detected mid-execution, apply the escalation:
- 2nd fail → mandatory approach pivot
- 3rd fail → 3-hypothesis test matrix
- 4th fail → zero-reset clarity checklist

## Verification protocol (MANDATORY per item)

See `references/verification-protocol.md` for the full protocol. Core rules:

1. **Choose the narrowest verifier** that can falsify the fix:
   - code change → run affected tests or lint
   - config change → app start / health check
   - data fix → query/diff the data
   - doc change → render or spell-check
2. **Capture tool output** — paste or summarize `stdout`/`stderr`.
3. **Binary verdict**: `✅ PASS` or `❌ FAIL` (with reason).
4. **On FAIL**: revert / isolate, mark item failed, continue to next independent item.
5. **Integration check** after all items: run full test suite / build once.

## Reference map

- [references/execution-workflow.md](references/execution-workflow.md) — queue template, summary template, decision rules
- [references/priority-classification.md](references/priority-classification.md) — severity→priority mapping, checkpoint format
- [references/verification-protocol.md](references/verification-protocol.md) — per-item verification steps, integration check, evidence standards
- [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) — shared finding / execution / verification schema

## Hard constraints

- Do not skip verification after a fix — every change must have tool-output evidence
- Do not batch unrelated fixes into one commit or one "done" claim
- Do not continue past a P0 failure without explicit user approval
- Do not modify files outside the scope of the current fix item
- Do not let parallel checklist items directly co-edit shared continuity artifacts
- Update the checklist source file with progress marks if it exists
- Always report what was fixed, what failed, and what remains
- When user selects items by number, execute ONLY those items
