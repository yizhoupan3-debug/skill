# Checklist Update Contract

Execution is not complete until the checklist is updated.

## Core rule

Treat checklist updates as part of the definition of done.
A checklist item is not fully complete if execution happened but the checklist still reflects the old state.

## Minimum required updates after execution

For every executed checklist point, write back at least:
- current status
- execution result
- verification result or evidence summary
- any newly discovered blocker or remaining gap

For the checklist as a whole, update at least:
- current status snapshot
- progress summary counts
- any changed execution-order note or dependency note

## Recommended per-point update shape

```markdown
### 执行结果
- 状态：`[ ]` / `[~]` / `[x]`
- 结果：<not started / in progress / done / blocked / failed>
- 验证：<test, command output, review note, or artifact evidence>
```

## Recommended document-level update shape

```markdown
## 当前状态快照
- <updated state summary>

## 当前完成统计模板
- 当前轮次总任务数：**<N>**
- 已完成：`<done>/<N>`
- 进行中：`<in-progress>/<N>`
- 未开始：`<not-started>/<N>`
```

## Status semantics

Use status values consistently:
- `[ ]` = not started
- `[~]` = in progress or partially complete
- `[x]` = complete

If execution failed or is blocked, say so explicitly in the execution result.
Do not mark `[x]` unless the acceptance criteria were actually met.

## When to update

Update the checklist:
- immediately after finishing a task point
- immediately after discovering a blocker that changes the plan
- after verification changes the status from tentative to confirmed
- at the end of the round to refresh summary counts

## What not to do

- Do not leave the checklist untouched after execution
- Do not mark a point complete without recording verification
- Do not hide blocked or failed work by leaving stale unchecked boxes
- Do not update only the local task and forget the top-level summary when counts changed
