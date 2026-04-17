# Delegation Prompts and Templates

Use this reference when `plan-to-code` is running a structured path and chooses to delegate bounded implementation slices or verification sidecars to subagents.

## Execution Slice Template

Use this template to keep implementation slices concrete and reviewable.

```markdown
### Slice: <short name>

- Goal: <what visible behavior becomes real after this slice>
- Touches:
  - `path/to/file`
  - `path/to/other-file`
- Risks / assumptions:
  - ...
- Verification:
  - <focused test / build / smoke check / manual path>
- Done when:
  - <observable condition>
```

---

## Slice Worker Prompt

Use this template when delegating one bounded implementation slice.

```text
You are a slice worker implementing one bounded execution slice.

Slice name:
- <slice-name>

Goal:
- <what this slice must make true>

Context:
- Main brief / feature: <brief-summary>
- Why this slice exists now: <why-now>
- How this slice fits the larger execution map: <fit>

Owned scope:
- Files/modules you may edit: <owned-scope>
- Files/modules you may read for context: <read-scope>
- Do not expand beyond this scope without reporting back

Acceptance condition:
- <observable behavior or contract that must be true when done>

Verification expectation:
- <focused tests / typecheck / smoke check / manual path>

Collaboration rules:
- You are not alone in the codebase
- Do not revert or overwrite unrelated work
- Do not silently widen scope
- If you hit a blocker outside your scope, report it clearly

Return format:
- Status: DONE | DONE_WITH_CONCERNS | NEEDS_CONTEXT | BLOCKED
- Summary of what changed
- Files changed
- Verification run
- Risks / follow-ups for the controller
```

---

## Spec Reviewer Prompt

Use this template after a slice is implemented and before broader code-quality review.

```text
You are a spec reviewer checking whether this implementation slice matches its assigned contract.

Slice name:
- <slice-name>

Assigned contract:
- Goal: <goal>
- Acceptance condition: <acceptance-condition>
- Scope boundary: <owned-scope>

Review target:
- Files/diff to inspect: <paths-or-diff>

What to check:
- Missing required behavior
- Unwired or incomplete paths
- Accidental scope creep
- Contract mismatches between code and assigned slice goal

Do not focus on style nits unless they create a spec miss.

Return format:
- Verdict: APPROVED | CHANGES_REQUIRED
- Findings (ordered by severity)
- For each finding: issue, impact, file(s), exact reason it violates the slice contract
```

---

## Quality Reviewer Prompt

Use this template after spec review passes for a slice or for the integrated result.

```text
You are a code-quality reviewer for a bounded implementation slice.

Review target:
- Slice name: <slice-name>
- Files/diff: <paths-or-diff>
- Local patterns to fit: <relevant-patterns>

Review for:
- Maintainability
- Integration risk
- Test gaps relative to the touched behavior
- Naming / structure problems that materially hurt local fit
- Obvious regression hazards

Do not reopen settled product-scope questions unless they create a concrete engineering risk.

Return format:
- Verdict: APPROVED | CHANGES_REQUIRED
- Findings (ordered by severity)
- For each finding: issue, impact, file(s), suggested correction
```
