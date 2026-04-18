# checklist-writting — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- turning ambiguous work into an execution-ready checklist
- making serial vs parallel structure explicit
- choosing practical verification and acceptance checks
- adapting checklist shape to the task type
- telling the user how many agents to start after writing the markdown

This skill does not own:
- implementing the full solution
- deep domain-specific architecture review by itself
- subagent orchestration policy

If the task shifts to adjacent skill territory, route to:
- `$plan-to-code` when the user wants the checklist executed into code
- `$checklist-normalizer` when the source is an existing messy checklist that needs reshaping
- `$subagent-delegation` when the main output should be a delegation strategy

## Finding-driven framework role

This skill is a **Phase-1 planner anchor** in the shared finding-driven
framework. When an upstream gate or detector has already produced findings, this
skill should turn them into an ordered, execution-ready checklist without re-doing the
diagnosis. Keep the checklist mappable to the shared contracts in
[`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md), but stay
lightweight when the user only wants a concise checklist.

## Required workflow

1. Extract the task shape:
   - object
   - action
   - constraints
   - deliverable
2. Decide the checklist points.
3. Treat peer points as parallel by default unless an explicit dependency says otherwise.
4. Keep any ordered serial chain inside one checklist point, no matter how long it is.
5. Make every point independently verifiable.
6. End with clear acceptance criteria and the agent-count callout.

## Core workflow

### 1. Identify the task type

Choose one primary task type:
- `new feature`
- `bug fix`
- `refactor`
- `audit / review`
- `migration / rollout`
- `research / document work`

### 2. Pick the right checklist shape

#### New feature
- goal
- affected surfaces
- parallel points
- serial steps inside each point
- validation

#### Bug fix
- reproduce
- isolate root cause
- apply minimal fix
- regression check

#### Refactor
- preserve behavior
- sequence risky changes inside the owning point
- verify no regressions

#### Audit / review
- inspect target
- collect evidence
- group findings into execution points
- define next actions

#### Migration / rollout
- prepare
- convert incrementally
- validate compatibility
- rollout / fallback

### 3. Write the checklist

Rules:
- prefer 3-8 checklist points unless the task genuinely needs more
- avoid fake parallelism
- every point must name:
  - the goal
  - the owned surface
  - the verification
- write the markdown under `checklist/cl_v<version>.md` unless the user explicitly wants chat-only output

Good point style:
- `Point 2 — Harden auth middleware` with ordered substeps and explicit verification.

Weak point style:
- `Fix auth`.

### 4. Add dependencies and risks only when useful

Include dependencies when:
- a later point is blocked by an earlier one
- multiple teams/filesystems/tools are involved
- rollout order matters

Include risks when:
- there is data loss risk
- there is production behavior risk
- there is API compatibility risk

### 5. Finish with acceptance and agent count

Always end with:
- a short overall acceptance line
- a direct statement of how many agents to start
- which points are parallel
- which steps remain serial inside one point

## Output defaults

Default output should contain:
- goal
- checklist points
- verification / acceptance
- agent-count note

Recommended structure:

````markdown
# <Checklist Title>

## Goal
- ...

## Parallel task summary
- Point 1: ...
- Point 2: ...

## Checklist
- [ ] Point 1: ...
  1. ...
  2. ...
  - Acceptance: ...
- [ ] Point 2: ...
  1. ...
  2. ...
  - Acceptance: ...

## Overall acceptance
- [ ] ...

## Agent count
- Start N agents.
````

## Review checklist workflow

When the user asks to review a checklist, this skill should first inspect the latest checklist file for execution state before deciding whether another round is needed.

Decision rules:
- `Continue` if any point remains `[ ]` or `[~]`
- `Continue` if any point lacks verification or evidence summary
- `Continue` if the checklist records blocked / failed work, residual risk, or newly discovered gaps
- `Converged` if all relevant points are `[x]`, verification is present, and no residual follow-up remains
- `Blocked` if the checklist shape is too incomplete to determine completion reliably

Review output rules:
- prefer reading the newest `cl_v*.md`
- if none exists, read the newest legacy `checklist_v*.md`
- base the decision on checklist status, execution result, verification/evidence, and any residual gap recorded in the document
- if decision is `Continue`, write the next round checklist as `checklist/cl_v<next>.md`
- if decision is `Converged`, explicitly tell the user the task is finished
- if decision is `Blocked`, tell the user what evidence or updates are missing

## Hard constraints

- Do not write a bloated pseudo-project-plan for a small task.
- Do not split one serial chain across multiple peer points just to create fake parallelism.
- Do not omit verification or acceptance criteria.
- Do not forget to tell the user how many agents to start.
- If the task is too ambiguous to shape responsibly, say what assumption you are making.

## Trigger examples

- "Use $checklist-writting to break this feature into actionable checklist points."
- "Before coding, give me a compact execution checklist with verification."
- "把这个重构拆成可执行 checklist，不要直接写代码。"
