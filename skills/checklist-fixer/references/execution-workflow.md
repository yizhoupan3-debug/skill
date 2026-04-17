# Checklist Fixer — Detailed Execution Workflow

Use this reference when the main `SKILL.md` is not enough and you need the
detailed queue/execution/checkpoint procedure.

## Accepted input forms

- markdown checklists (`- [ ]`)
- numbered issue lists
- priority-tagged items (`[P0]`, `[P1]`, `[CRITICAL]`)
- upstream findings ledgers
- `implementation_plan.md` proposed-changes sections
- user selection by number ("1", "2 and 3", "just the first one")

For each item, preserve:

- item ID
- source findings
- native severity
- normalized priority
- location
- verification method
- dependencies

## Item selection from user reply

When the user gives a minimal reply (number, range, brief confirmation),
detect which items to execute before building the queue:

| User reply pattern | Action |
|---|---|
| `"1"`, `"item 1"`, `"第一个"` | Execute only item 1 |
| `"1和3"`, `"1, 3"`, `"1+3"` | Execute items 1 and 3 only |
| `"1-3"`, `"前三个"` | Execute items 1, 2, 3 |
| `"好"`, `"ok"`, `"可以"`, `"确认"` | Execute all items in priority order |
| `"先做P0"`, `"从P0开始"` | Execute all P0 items first |
| `"只做 X"` | Execute only item X |

If selection is genuinely ambiguous, ask once: "执行全部 N 项，还是指定几个？"

## Queue construction

Order by:

1. priority (P0 → P1 → P2 → P3)
2. dependency (blocked items last)
3. independence / grouping safety

### Queue template

```markdown
## Fix Queue (N items)
| # | Priority | Description | Deps | Status |
|---|----------|-------------|------|--------|
| 1 | P0       | ...         | —    | pending |
```

## Execution loop

For each item:

1. understand the exact change required
2. implement the smallest correct fix (no scope creep)
3. **verify with tool output** — see `references/verification-protocol.md`
4. update checklist / checkpoint with `✅` or `❌`
5. commit with traceability if requested

**Anti-laziness checkpoints (mandatory each iteration):**
- Did I produce real tool output, not a claim?
- Is the code complete, without `...` or placeholders?
- Did I avoid touching files outside this item's scope?

## Failure / blocked handling

If verification fails:

1. Revert or isolate the broken change
2. Mark the item failed with reason
3. Continue only if the next item is independent
4. Route unexplained failures to `$systematic-debugging`

If blocked:

1. Mark blocked with dependency or investigation reason
2. Skip to the next unblocked item

## Completion summary template

```markdown
## Fix Execution Summary

### Completed (N items)
- ✅ #1 — <description> — verified: <evidence>
- ✅ #2 — ...

### Failed (N items)
- ❌ #3 — <description> — reason: <failure detail>

### Skipped / Blocked
- ⏭ #4 — blocked on: <reason>

### Integration Check
- Tests:  PASS (N/N) / FAIL
- Build:  PASS / FAIL
- Lint:   PASS / N warnings

### Residual Risk
- <any regressions or follow-up items found>
```

## Decision rules

- if a fix becomes architectural, consider `$refactoring`
- if root cause becomes unclear, route that item to `$systematic-debugging`
- if only P2/P3 remain and user hasn't asked for them, ask whether to continue
- for >20 items, work in waves of ≤10 with verification checkpoints between waves
- **always run a false-convergence scan** after completing the queue

## Upstream sources

- `plan-writing` / `implementation_plan.md`
- `code-review`
- `architect-review`
- `accessibility-auditor`
- `security-audit`
- `systematic-debugging`
- `paper-notation-audit`, `paper-logic` (academic upstream)
- user-authored checklists
