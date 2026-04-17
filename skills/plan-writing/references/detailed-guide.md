# plan-writing — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- turning ambiguous work into a concise task list
- identifying dependencies and ordering
- choosing practical verification steps
- adapting the plan shape to the task type

This skill does not own:
- implementing the full solution
- deep domain-specific architecture review by itself
- subagent orchestration policy

If the task shifts to adjacent skill territory, route to:
- `$plan-to-code` when the user wants the plan executed into code
- `$subagent-delegation` when the main output should be a delegation strategy

## Finding-driven framework role

This skill is a **Phase-1 planner anchor** in the shared finding-driven
framework. When an upstream gate or detector has already produced findings, this
skill should turn them into an ordered execution queue without re-doing the
diagnosis. Keep the plan mappable to the shared contracts in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md) and
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md), but stay
lightweight when the user only wants a concise plan.

## Required workflow

1. Extract the task shape:
   - object
   - action
   - constraints
   - deliverable
2. Choose the right plan shape for the task type.
3. Write only the minimum number of steps needed.
4. Make every task independently verifiable.
5. End with clear success criteria.

## Core workflow

### 1. Identify the task type

Choose one primary task type:
- `new feature`
- `bug fix`
- `refactor`
- `audit / review`
- `migration / rollout`
- `research / document work`

### 2. Pick the right plan shape

#### New feature
- goal
- files/systems affected
- build steps
- validation

#### Bug fix
- reproduce
- isolate root cause
- apply minimal fix
- regression check

#### Refactor
- preserve behavior
- sequence risky changes
- verify no regressions

#### Audit / review
- inspect target
- collect evidence
- group findings
- define next actions

#### Migration / rollout
- prepare
- convert incrementally
- validate compatibility
- rollout / fallback

### 3. Write the task list

Rules:
- prefer 3-8 tasks
- avoid micro-steps unless risk is high
- every task must name:
  - the action
  - the target
  - the verification

Good task style:
- `Update auth middleware to enforce org scope → Verify: request without org access returns 403`

Weak task style:
- `Fix auth`

### 4. Add dependencies and risks only when useful

Include dependencies when:
- a later step is blocked by an earlier one
- multiple teams/filesystems/tools are involved
- rollout order matters

Include risks when:
- there is data loss risk
- there is production behavior risk
- there is API compatibility risk

### 5. Finish with success criteria

Always end with a short `Done when` section.

## Output defaults

Default output should contain:
- goal
- tasks
- verification
- done-when criteria

Recommended structure:

````markdown
# <Plan Title>

## Goal
- ...

## Tasks
- [ ] Task 1: ... → Verify: ...
- [ ] Task 2: ... → Verify: ...
- [ ] Task 3: ... → Verify: ...

## Risks / Dependencies
- ...

## Done When
- [ ] ...
````

## Hard constraints

- Do not write a bloated pseudo-project-plan for a small task.
- Do not use generic tasks like "implement feature" without naming the target and verification.
- Do not exceed the complexity the user needs.
- Do not omit verification criteria.
- If the task is too ambiguous to plan responsibly, say what assumption you are making.

## Trigger examples

- "Use $plan-writing to break this feature into actionable tasks."
- "Before coding, give me a compact implementation plan with verification."
- "把这个重构拆成可执行计划，不要直接写代码。"
