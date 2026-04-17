# Subagent Delegation — Runtime Playbook

Use this reference when the main `SKILL.md` is not enough and you need the detailed delegation decision path, runtime patterns, fallback rules, or output templates.

## Core rule

Treat delegation as a **structure decision first** and a **spawn decision second**.

- First decide whether the task wants bounded sidecars.
- Then record that sidecar plan in state or artifacts.
- Then decide whether the current runtime can actually spawn them.
- If it cannot, preserve the same sidecar structure in local-supervisor mode.

## Strong signals that usually justify delegation

- “figure out the bug, implement the fix, and verify it”
- “make a large repo change across several modules”
- “research options while implementation moves”
- “review a slice while another slice is being built”
- “collect evidence from multiple bounded surfaces”

## Strong signals that usually argue against delegation

- the task is tiny
- the task is vague
- the immediate blocker is faster to solve locally
- multiple workers would need overlapping write scopes
- the main thread needs the answer immediately before anything else can continue

## Decision path

### 1. Decide the main-thread next step first

Before any delegation choice, answer:

- what is the immediate critical-path action?
- what can be safely sidecarred?
- what must remain local?

### 2. Define sidecar boundaries

Good sidecars usually have:

- one target surface
- one role
- one clear output contract
- no hidden write overlap

### 3. Record the delegation plan

Before any runtime branch, persist at least:

- `delegation_plan_created`
- candidate sidecars and output contracts
- what stays local

### 4. Check runtime policy

If runtime policy **permits** spawning:

- attempt spawning from the saved plan
- dispatch bounded sidecars
- keep orchestration and synthesis local

If runtime policy does **not** permit spawning:

- keep the same sidecar plan
- convert it into a local-supervisor queue
- execute slices sequentially or in compact local loops
- report them as queue items, not as sprawling chat narration

## Main-thread compression

Keep the main thread to:

- why delegation structure was or was not justified
- whether spawning happened or local-supervisor fallback was used
- what stays local
- what the next integration step is

Do **not** use the main thread for:

- full worker prompts
- raw logs
- long repeated reasoning already stored elsewhere

## After delegation or queueing

### 5. Keep the main thread moving

After delegation or queue creation:

- continue local non-overlapping work
- avoid reflexive waiting
- only wait when the critical path truly blocks

### 6. Review and integrate

When sidecars return, or when queued local slices finish:

- check whether the assigned task was answered
- check for boundary violations
- integrate only the result summary and proof
- keep raw detail outside the main thread

## Runtime output template

````markdown
## Delegation Summary
- Main-thread goal: ...
- Delegation plan created: yes / no
- Why delegation structure was/was not used: ...
- Runtime mode: spawned sidecars / local-supervisor fallback

## Delegated or Queued Work
1. Task: ...
   - Role: ...
   - Scope: ...
   - Output: ...

## Kept Local
- ...

## Main Thread Now
- ...
````

## Planning-only template

````markdown
## Delegation Plan Summary
- Goal: ...
- Critical path: ...
- Runtime mode assumption: ...

## Main Thread Now
- ...

## Suggested Sidecars or Local Queue
1. Task: ...
   - Role: ...
   - Why: ...
   - Output contract: ...
````
