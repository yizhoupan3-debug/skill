---
name: assignment-compliance
description: |
  Check whether a homework or course-project submission satisfies every
  requirement in the problem statement or rubric.
  Use when the user wants line-by-line requirement extraction from PDFs, screenshots, rubrics,
  or mixed sources, then map them to the student work and produce a compliance
  checklist, gap analysis, score estimate, and fix plan. Trigger phrases
  include “检查作业要求”, “对照 rubric”, “check assignment requirements”,
  “能拿满分吗”, and “还差什么”.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "4.0.0"
  platforms: [antigravity, codex]
  tags: [assignment, homework, project, rubric, compliance, checklist, grading, requirements, submission]
risk: safe
source: local
---

# Assignment Compliance Checker

This skill owns **requirement-by-requirement compliance verification** for
homework assignments and course projects.

## When to use

- The user wants to verify an assignment/project meets **every** requirement
- The user has a problem statement, rubric, or grading criteria to check against
- The user asks whether submission is complete and ready to turn in
- The user wants a structured gap analysis before deadline
- The user wants to catch missing or partially-met requirements
- The user wants to maximize score by ensuring full rubric coverage
- The user asks "还差什么", "能拿满分吗", "扣分点在哪"

## Do not use

- The user wants a full paper-level reviewer critique → use `$paper-reviewer`
- The task is only wording/writing polish → use `$paper-writing`
- The task is scientific logic or claims-vs-evidence review → use `$paper-logic`
- The task is only code quality review → use `$coding-standards`
- The task is debugging code errors → use `$systematic-debugging`
- The user wants to actually write or fix content (not check) → use domain-specific skill
- The user only wants to proofread formatting → use `$pdf` or `$doc`

## Task ownership and boundaries

This skill owns:
- Requirement extraction from problem statements, rubrics, and grading criteria
- Per-requirement compliance mapping and judgment
- Implicit requirement mining (professor expectations not stated explicitly)
- Gap analysis and prioritized fix plans
- Cross-audit consistency checks
- Compliance summary and readiness verdict

This skill does not own:
- Actually rewriting content to fix gaps (delegate to appropriate skill)
- Deep scientific logic review (delegate to `$paper-logic`)
- Code rewriting or debugging

## Required workflow

1. Gather **all** requirement sources first; use `$pdf` / `$visual-review` for unread artifacts.
2. Extract atomic requirements, preserve original wording, and classify each as hard / scored / implicit / bonus.
3. Inventory deliverables and map evidence to each requirement.
4. Judge each requirement as ✅ PASS / ⚠️ PARTIAL / ❌ FAIL / 🔍 UNCLEAR with evidence.
5. Run cross-audit checks and prioritize gaps into P0 / P1 / P2.
6. Produce a compliance table, verdict, score estimate (if possible), and top fix priorities.

## Reference map

- [references/compliance-workflow.md](references/compliance-workflow.md) — detailed multi-source workflow, summary template, incremental re-check mode, pitfalls, collaboration
- [references/check-dimensions.md](references/check-dimensions.md) — type-specific check tables
- [references/checklist-template.md](references/checklist-template.md) — reusable compliance checklist skeleton

## Hard constraints

- Check **every** requirement; never silently skip or merge independent items
- If evidence is missing, use 🔍 UNCLEAR instead of assuming compliance
- If requirement sources conflict or are critically ambiguous, stop and ask
- Never fabricate evidence locations, counts, or extracted requirement text
- Be explicit and concrete; avoid vague reassurance
