---
name: checklist-normalizer
description: |
  Normalize an existing checklist or execution plan into an execution-ready shape.
  Use for 规范化 checklist, clarifying serial vs parallel boundaries, filling goals/constraints/acceptance,
  and adding explicit update rules after execution.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - 规范化 checklist
  - 串行的写在一点
  - 并行的拆开
  - lane 重写
  - 补齐验收和约束
  - 整理成可执行清单
short_description: Rewrite a messy checklist into an execution-ready form with explicit serial/parallel boundaries and acceptance rules.
metadata:
  version: "1.1.0"
  platforms: [codex, claude]
  tags:
    - checklist
    - normalization
    - execution-plan
    - parallel-lanes
    - acceptance-criteria
    - progress-tracking
risk: low
source: local
---

# Checklist Normalizer

This skill owns **checklist shape normalization**: take an existing checklist,
execution blueprint, phase plan, experiment roadmap, claim-closure plan, or
multi-agent task plan and rewrite it into a stable, execution-ready checklist
with explicit point boundaries, clear serial grouping, point-level parallel
isolation, explicit goals/constraints/acceptance, and mandatory checklist
update rules.

## When to use

- The user asks to 规范化 checklist or 整理成可执行清单
- The checklist mixes serial steps and parallel work without clear boundaries
- The user wants 串行步骤写在一点 and 并行步骤拆开写
- The user wants the checklist written in a point-based form where peer points are treated as parallel by default
- The user wants a long serial sequence kept inside one point instead of being scattered into fake peer bullets
- The checklist is missing goals, constraints, deliverables, acceptance, exit conditions, or stop conditions
- The user wants a phase checklist, experiment roadmap, claim-closure plan, or multi-agent plan rewritten into execution-ready checklist form
- The user wants agent grouping made explicit while preserving point-level isolation
- The user wants the checklist to specify how progress must be updated after execution
- A plan already exists, but it is not yet shaped like an execution-ready checklist

## Do not use

- The user wants to execute checklist items right now → `$checklist-fixer`
- The user only has a goal/spec and no checklist or draft checklist yet → `$checklist-writting`
- Root cause is unknown and investigation is still needed → `$systematic-debugging`
- The task is code implementation from a PRD/spec instead of checklist normalization → `$plan-to-code`
- The request is about writing or upgrading SKILL.md files rather than task checklists → `$skill-writer` or `$writing-skills`

## Task ownership and boundaries

This skill owns:
- extracting the true task shape from a loose or inconsistent checklist
- treating peer checklist points as parallel by default unless the checklist explicitly marks a dependency
- grouping any serial work into one checklist point with ordered substeps, even when the serial chain is long
- splitting independent work into separate isolated points or lanes
- adding missing fields such as current state, goal, constraints, deliverables, acceptance, exit conditions, and stop conditions
- making execution boundaries explicit with exclusive scope and forbidden scope when needed
- defining how checklist progress must be written back after execution
- producing a checklist that can be handed to execution skills without structural ambiguity

This skill does not own:
- executing checklist items
- fixing code or configs directly
- discovering unknown bugs or root causes
- inventing a feature plan from scratch when no checklist exists yet

If the task shifts from reshaping the checklist to carrying it out, route to:
- `$checklist-fixer`

## Required workflow

1. **Extract task shape**
   - identify object, action, constraints, and deliverable
   - detect whether the source is a checklist, phase blueprint, issue list, or mixed plan
2. **Classify the source material**
   - separate serial steps, parallel work, background state, and out-of-scope items
3. **Normalize the structure**
   - treat peer points as parallel by default unless there is an explicit dependency
   - merge every serial chain into one point with ordered substeps, no matter how long the serial chain is
   - split truly independent work into separate points or lanes with explicit boundaries
4. **Fill the required fields**
   - make sure each point has current state, goal, constraints, deliverables, acceptance, exit conditions, and stop conditions when they matter
5. **Add closure rules**
   - state how execution must update status, execution results, verification results, and progress counts
6. **Deliver the rewritten checklist**
   - preserve known facts
   - explicitly mark missing information instead of inventing it

## Checklist shape rules

Apply these rules by default unless the user asks for a lighter format:

- **Peer checklist points are parallel by default** unless an explicit dependency or ordering note says otherwise.
- **Peer task points are parallel by default** unless an explicit dependency note says otherwise.
- **Serial work belongs in one point** when the work shares one goal, one owner lane, one primary surface, and must happen in order — even if the serial chain is long.
- **Each parallel point must be clearly isolated**. When relevant, write its exclusive scope and forbidden scope.
- **Shared continuity artifacts are not parallel write surfaces.** By default, non-integrator points must forbid direct writes to root mirrors and shared pointers: `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`, `.supervisor_state.json`, `artifacts/current/active_task.json`, and `artifacts/current/focus_task.json`.
- **Task-scoped continuity is the primary truth.** Prefer `artifacts/current/<task_id>/` for task body content, and treat `artifacts/current/task_registry.json` as workspace indexing only.
- **If global continuity must be refreshed**, create a dedicated integrator point instead of letting peer points co-edit the shared focus projection.
- **Add exit conditions and stop conditions** whenever the point represents a bounded execution track, experiment lane, or agent-owned workstream.
- **Every execution-ready checklist should include top-level structure** for:
  - overall goal
  - current status snapshot
  - parallel task summary
  - recommended execution order
  - items not included in this round
  - overall acceptance line
  - progress summary template
- **Execution completion must update the checklist**. Treat checklist updates as part of done-ness, not an optional follow-up.

## Output defaults

Default output should contain:
- a short normalization summary
- the rewritten checklist structure
- explicit missing-information notes if some required fields cannot be derived

Recommended structure:

````markdown
## Normalization summary
- what was regrouped
- what was split into parallel lanes
- what fields were added or still missing

## Rewritten checklist
- execution-ready checklist in normalized shape

## Open gaps
- facts the user still needs to supply
````

## Hard constraints

- Do not leave serial dependencies scattered across multiple peer bullets
- Do not split one serial chain across several peer points just because it is long
- Do not mix parallel work into one checklist point when isolation is needed
- Do not omit goals, constraints, acceptance criteria, exit conditions, or stop conditions from an execution-ready checklist when they are needed to bound work
- Do not invent facts that are not present in the source material
- Do not treat checklist updates after execution as optional
- Do not turn normalization into execution; stop at the rewritten checklist unless the user asks to continue

## File-output rule

Unless the user explicitly asks for chat-only output:

- write the normalized checklist under `checklist/`
- create `checklist/` if it does not exist
- prefer continuing the existing `cl_v*.md` series; otherwise start from `checklist/cl_v1.md`
- explicitly label which peer points are parallel and which ordered substeps stay serial inside one point

## Trigger examples

- "把这份 checklist 规范一下，串行的写在一点，并行的拆开。"
- "帮我把这个 phase plan 整理成可执行清单，补齐验收和约束。"
- "这份任务列表太乱了，按 lane 重写一下，并写明做完后怎么更新 checklist。"
- "把这个 execution blueprint 改成标准 checklist 形态。"

## Reference map

- [references/checklist-shape-spec.md](references/checklist-shape-spec.md) — required checklist fields and normalization rules
- [references/checklist-template.md](references/checklist-template.md) — reusable normalized checklist template
- [references/parallel-vs-serial-guidelines.md](references/parallel-vs-serial-guidelines.md) — how to decide whether work stays in one point or splits into lanes
- [references/checklist-update-contract.md](references/checklist-update-contract.md) — what must be written back after execution
