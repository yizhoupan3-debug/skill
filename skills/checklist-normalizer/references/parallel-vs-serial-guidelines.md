# Parallel vs Serial Guidelines

Use this guide to decide whether work should stay inside one checklist point or split into separate points.

## Serial work belongs in one point when

Keep work in one checklist point when all of the following are true:
- the steps share one goal
- they should be owned by the same execution lane
- they touch the same primary surface
- they must happen in order
- splitting them would create fake parallelism
- the serial chain may be long, but it is still one bounded execution point

### Recommended shape

Use one task point with ordered substeps:

```markdown
## T1. <Task>

### 串行步骤
1. ...
2. ...
3. ...
```

### Typical examples
- define contract → implement contract → add focused regression
- prepare migration inventory → update source of truth → rewrite dependent docs
- add schema field → wire emitter → update consuming tests

## Parallel work must split into different points when

Treat peer checklist points as parallel by default unless a dependency note says otherwise.

Split work into separate checklist points when any of the following is true:
- items can make progress independently
- different owners or sidecars may take them
- they touch different file families or surfaces
- they would compete for one checklist slot and hide boundary decisions
- one item should treat another as read-only context rather than shared ownership

### Recommended shape

Use separate points or lanes and declare boundaries:

```markdown
## T1. <Lane A>
### 独占写入范围
- ...
### 禁止越界
- ...

## T2. <Lane B>
### 独占写入范围
- ...
### 禁止越界
- ...
```

### Typical examples
- storage lane vs trace lane
- adapter lane vs artifact lane
- desktop host lane vs CLI host lane
- contract lane vs compatibility-deprecation lane

## Boundary-writing rules

When work is parallelized, make the isolation visible:
- name the lane or task explicitly
- state the owned surface
- state what must remain untouched
- note any read-only dependency on another lane
- keep acceptance criteria local to that point

## Smells that usually mean the checklist is wrong

### Fake serial split
A checklist uses several peer bullets, but they are really one ordered sequence.

Fix: merge them into one point and add ordered substeps.

### Hidden parallelism
One bullet says "do A, B, C" but A, B, and C live in different execution lanes.

Fix: split them into different task points.

### Shared-ownership blur
Two points both appear allowed to change the same surface.

Fix: reassign ownership or declare one lane read-only.

### Missing dependency note
Two parallel points are separate, but one silently depends on the other.

Fix: keep them separate, but document the dependency as a read-only prerequisite or ordering note.
