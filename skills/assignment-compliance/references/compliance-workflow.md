# Assignment Compliance — Detailed Workflow

Use this reference when the main `SKILL.md` is not enough and you need the
full compliance-check procedure or output template.

## Multi-source gathering

Requirements may be scattered across:

- assignment sheet / problem statement
- screenshots, scans, or handwritten photos
- syllabus
- rubric / grading criteria
- LMS announcements
- professor emails or clarifications

### `problem/` directory convention

If a `problem/` directory exists in the project root, treat every file inside
it as an authoritative requirement source. Do not skip image files.

### Extraction method by format

| Format | Extraction method |
|---|---|
| PDF | `$pdf` |
| Image (`png/jpg/jpeg/heic/webp`) | `$visual-review` |
| Text / Markdown | read directly |
| DOCX / PPTX | convert/read with the appropriate tool |

If sources conflict, halt and ask the user which source takes priority.

## Requirement extraction

1. Number each atomic requirement as `R1`, `R2`, ...
2. Preserve the original wording alongside the normalized description
3. Classify each requirement:
   - **Hard**
   - **Scored**
   - **Implicit**
   - **Bonus**
4. Preserve explicit point values if present
5. Split multi-part sentences into separate requirements

### Implicit requirement mining

Actively scan for likely unstated expectations:

- citation style
- student name / ID / filename requirements
- conclusion / references / appendix expectations
- code comments / README / reproducibility
- figure numbering / captions / legends
- units / significant digits / formatting rules

## Deliverable inventory

List all submitted artifacts and map each one to the requirements it satisfies.
Flag:

- orphan deliverables
- orphan requirements
- partial evidence

## Per-requirement judgment

Use:

- ✅ PASS
- ⚠️ PARTIAL
- ❌ FAIL
- 🔍 UNCLEAR

Always cite evidence precisely. If evidence is missing, use 🔍 UNCLEAR.

## Cross-audit checks

- requirement coverage completeness
- internal consistency
- formatting consistency
- reference-content alignment
- figure-text alignment
- submission packaging correctness
- deadline / submission method compliance

## Summary output template

```markdown
## 合规总表

| Req | Category | Pts | Description | Status | Priority | Notes |
|-----|----------|-----|-------------|--------|----------|-------|
| R1  | Hard     | 10  | ...         | ✅     | —        | ...   |
| R2  | Scored   | 15  | ...         | ⚠️     | P1       | ~10/15 |
| R3  | Hard     | 10  | ...         | ❌     | P0       | 0/10  |

## Stats
- Compliance rate: X / Y
- P0 gaps: N
- P1 gaps: N
- P2 gaps: N

## Score Estimation
- Estimated current: ~82
- Points at risk: ~18

## Verdict
✅ Ready to submit / ❌ Not ready

## Top Fix Priorities
1. ...
2. ...
3. ...
```

If the rubric has no points, omit score estimation.

## Incremental re-check mode

When the user has fixed only part of the work:

1. load the prior table
2. identify affected requirements
3. re-check only affected items plus dependent cross-audits
4. keep unaffected judgments unchanged
5. highlight deltas such as `R3: ❌ → ✅`

## Common pitfalls

- page/word count off by one
- missing student ID / filename requirement
- wrong citation style
- figures present but unlabeled
- missing sub-question parts
- wrong output / submission format
- missing comparison / baseline explicitly asked for

## Collaboration map

- requirement PDFs → `$pdf`
- image-based requirements → `$visual-review`
- paper deliverables → `$paper-reviewer`, `$paper-writing`, `$paper-visuals`
- code deliverables → `$coding-standards`, `$test-engineering`
- slides -> `$slides` first, then `$ppt-beamer` / `$ppt-pptx` / `$source-slide-formats` as needed
- notation / symbols → `$paper-notation-audit`
