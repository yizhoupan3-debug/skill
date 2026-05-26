# Rubric / Bonus Audit Bridge

Optional branch when `$paper-workbench` detects assignment text, grading rubric,
**Bonus** criteria, or explicit 「题目要求 / 评分标准」 in the user message.

Does **not** revive `$assignment-compliance` as a user entry — workbench enables
this slice inside exhaustive review only.

## When to enable

- User pasted rubric, syllabus requirements, or project spec alongside the manuscript
- User asks for completion check against stated requirements or Bonus items
- `audit_depth: exhaustive` and requirement-like text is present

## Requirement decomposition

Before merging dimension findings:

1. Extract every **required** item and every **Bonus** item into a table.
2. Assign stable ids:
   - Required: `req_001`, `req_002`, …
   - Bonus: `bonus_001`, `bonus_002`, …
3. For each row, compare manuscript against requirement text.

## requirement_matrix shape

```text
requirement_matrix:
  - req_id: req_001
    text: (requirement verbatim or short paraphrase)
    status: done | partial | missing
    evidence_location: (section / figure / appendix — or "none")
    severity: P0 | A | B | Warning | C
  - req_id: bonus_001
    text: (Bonus criterion)
    status: done | partial | missing
    evidence_location: ...
    severity: ...
```

## Merge with dimension findings

- **Do not duplicate**: if a gap is already a strategic/math/language finding with
  the same root cause, cross-link `req_id` in that finding's `id` or `issue` field.
- **Bonus missing** → default severity **Warning** unless the rubric marks it as
  required for full credit (then **B**).
- **Required missing** → at least **B**; if it invalidates the core deliverable, **A**.

## Output placement

Include `requirement_matrix` in the exhaustive envelope defined in
[`paper-exhaustive-audit.md`](paper-exhaustive-audit.md) — not as a separate report type.
