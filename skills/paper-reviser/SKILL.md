---
name: paper-reviser
description: |
  Revise an academic paper or manuscript from reviewer comments, issue lists, rebuttal
  goals, or submission-readiness gaps across logic, writing, visuals, and layout. Sole
  owner of rebuttal / response-letter orchestration. Use when the user asks '按这些意见帮我改',
  '修到能投', '根据 reviewer comments 修改', '写 rebuttal', or wants coordinated revision.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 按这些意见帮我改
  - 修到能投
  - 根据 reviewer comments 修改
  - 写 rebuttal
  - coordinated revision
  - paper
  - manuscript
  - revise
  - fix
  - reviewer comments
metadata:
  version: "2.1.1"
  platforms: [codex]
  tags: [paper, manuscript, revise, fix, reviewer-comments, submission, rebuttal, response-letter]
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

# Paper Reviser

This skill owns **whole-paper revision orchestration** and **reviewer response management**.

## When to use

- The user wants manuscript changes, not just review comments
- The user provides reviewer comments or a problem list
- The task spans multiple paper surfaces such as logic + writing + visuals
- The user wants coordinated revisions toward a submission bar
- The user wants to write a rebuttal or response letter

## Do not use

- The user only wants a reviewer-style assessment → use `$paper-reviewer`
- The task is only logic revision → use `$paper-logic`
- The task is only prose revision → use `$paper-writing`
- The task is only figure/table presentation revision → use `$paper-visuals`

## Task ownership and boundaries

This skill owns:
- issue-list driven paper revision
- sequencing fixes across subdomains
- recheck planning after edits
- deciding when to call specialized paper skills
- **rebuttal / response letter drafting when it involves coordinating actual manuscript edits**
- revision diff tracking

This skill does not own:
- rebuttal or response letter that is **purely about writing quality** without manuscript edits → `$paper-writing`

> **Rebuttal routing rule**: "帮我写 rebuttal" / "写 response letter" → always start with `paper-reviser`. If the user truly only wants prose polish on an already-drafted response letter, `paper-reviser` delegates to `paper-writing`.

## Finding-driven framework role

This skill is a **Phase-1 planner / executor / verifier anchor** in the shared finding-driven framework. It consumes structured findings from upstream paper review skills or reviewer comments normalized into the shared schema in [`../SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md).

Before editing, normalize raw reviewer comments into finding entries whenever possible. Preserve richer upstream fields instead of flattening them away. For each execution batch, materialize queue items with:
- `execution_item_id`
- `source_findings`
- `owner_skill`
- `executor_skill`
- `change_scope`
- `verification_strategy`
- `recheck_scope`
- `status`

After edits, update finding status to `resolved`, `partial`, or `blocked`, and record residual risk explicitly. When incoming findings include `repair_leverage`, preserve it through execution planning and final reporting rather than collapsing it into severity.

## Required workflow

### Phase 1: Issue Intake

> This skill is typically driven by output from `$paper-reviewer` (issue list) or external reviewer comments. When no structured issue list exists, derive one first with `$paper-reviewer`.

1. Start from reviewer comments or derive an issue list with `$paper-reviewer`.
2. Preserve incoming `finding_id` values when present, including dimension-coded IDs such as `NOV-01`, `THY-02`, `EXP-03`, `RES-04`, `WRT-05`, `REF-06`, and `VIS-07`; normalize only missing fields into finding entries with `severity`, `fixability`, recommended owner/executor skill, and `verification_method`.
3. Preserve and consume upstream planning hints when present, especially `repair_leverage`, shortest repair path, and any note that the best next step is to reorganize underexploited evidence already present instead of generating entirely new content.
4. Group issues by owner:
   - logic → `$paper-logic`
   - writing → `$paper-writing`
   - visuals → `$paper-visuals`
   - rendered layout → `$pdf`
5. Build an execution queue. For each batch, record: `execution_item_id`, `source_findings`, executor, change scope, verification strategy, and recheck scope.

### Phase 2: Surgical Execution

6. Fix highest-risk issues first (P0 → A → B → C), using `repair_leverage` to break ties inside the same severity bucket so high-yield / low-cost revisions land first.
7. If the shortest credible repair path says the issue can be solved by reorganizing underexploited evidence already present in the manuscript, appendix, figures, tables, or analysis drafts, prefer that path before requesting genuinely new experiments or assets.
8. **Execution discipline**: only modify regions directly related to the fix. All other text, structure, and punctuation remain frozen.
9. Severity markers determine depth:
   - Single marker (✅) = standard fix
   - Multiple markers (✅✅✅) = deep rewrite and thorough rework of that point
10. After every 3 fixes, pause and verify:
   - Was the fix actually applied correctly?
   - Did the fix introduce any side effects?
   - **Context Integrity**: Did the surgical edit leave orphaned text or create a duplicate heading (e.g., `\paragraph`)?
   - If not applied correctly, redo before continuing.

### Phase 3: Reflective Recheck

9. After all fixes are applied, **re-run the review** on the edited artifact:
   - Do not check from memory; re-read the actual edited text
   - Use the same Tier-1/2/3 checklist from `$paper-reviewer`
   - **Structural Scan**: Run a global search for duplicate headers (Sections, Subsections, Paragraphs). Use `grep` or similar tools to ensure each `\paragraph{Name}` is unique.
   - Focus especially on whether fixes introduced new inconsistencies

10. **Cross-fix interference check**:
   - Did fix A contradict or weaken fix B?
   - Did any fix change a claim without updating the corresponding evidence?
   - Did any fix break symbol consistency, figure numbering, or cross-references?

11. If the recheck surfaces new issues:
    - Classify them as `fix-induced` (caused by the revision) vs `pre-existing` (missed earlier)
    - Fix `fix-induced` issues immediately
    - Report `pre-existing` issues in the output as newly discovered

12. For each resolved or unresolved item, record `status`, `verification_method`, and `remaining_risk`.
13. Report remaining blockers honestly. Include a **confidence statement**:
    - "All P0 and A issues resolved with high confidence"
    - "N issues remain at medium confidence — recommend human verification of [specific point]"

### Phase 4: Rebuttal / Response Letter (when applicable)

14. Structure response by reviewer:
    - Quote each reviewer comment
    - State the action taken (with page/line references)
    - Highlight the key evidence or change
    - If declining a suggestion, give clear technical rationale
15. Response letter tone: respectful, specific, evidence-linked, non-defensive.
16. Use diff-style formatting or color markup to show changes in the manuscript.

## Output defaults

### For revisions: `论文修订记录`

| Finding ID | Exec ID | Severity | Repair Leverage | Action Taken | Owner Skill | Verification | Remaining Risk | Status |
|---|---|---|---|---|---|---|---|---|
| NOV-01 | EXEC-01 | P0/A/B/C | high/medium/low | ... | ... | ... | ... | 已解决/部分解决/受阻 |

When useful, add one short line under an entry noting whether the fix used `existing evidence reorganized` or `new evidence created`.

### For rebuttal: `审稿意见回复`

```markdown
## Reviewer #N

> [Reviewer comment quoted]

**Response**: [Action taken and rationale]
- Changed: [specific location and edit summary]
- Evidence: [what supports this change]
```

## Hard constraints

- Do not fabricate experiments, numbers, or evidence.
- Do not treat cosmetic edits as resolution of scientific issues.
- When layout or visual quality matters, verify rendered artifacts.
- Do not modify text outside the targeted fix region without explicit authorization.
- **No Structural Duplication**: Proactively ensure that no `\section`, `\subsection`, or `\paragraph` name is duplicated in the document.
- After completing all revisions, clean up all working notes and produce only the final clean output.
