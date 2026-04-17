---
name: paper-notation-audit
description: |
  Audit and enforce notation consistency across an academic paper: abbreviations,
  symbol definitions, formula numbering, cross-references, and unit / dimension
  consistency.
  Use when the user asks “检查缩写”, “缩写第一次要全称”, “符号有没有统一”, “符号冲突”,
  “公式编号对不对”, “方程引用错了”, “notation audit”, “acronym”, or wants a
  systematic notation sweep rather than logic or prose review.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags: [paper, notation, symbol, abbreviation, formula, unit, dimension, audit, acronym, theorem]
framework_roles:
  - detector
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: low
source: local
---

# Paper Notation Audit

This skill owns **notation-level consistency auditing** for academic papers:
symbols, abbreviations, formulas, units, and their cross-references.

## Finding-driven framework compatibility

Notation findings should be mappable to the shared finding-driven framework
while keeping symbol-level precision intact.

Minimum compatibility expectations:
- preserve `finding_id` for repeated symbol/notation issues
- keep `severity_native` in paper terms (`P0 / A / B / C`)
- include `evidence`, `fixability`, and `recommended_owner_skill`
- when the issue was surfaced by `$paper-reviewer` or `$paper-logic`,
  consume the incoming finding rather than re-discovering the same notation
  issue from scratch

## When to use

- The user wants a notation consistency sweep
- The user asks whether abbreviations are properly expanded at first use
- The user wants to verify symbol definitions and uniqueness
- The user wants formula numbering and cross-reference checks
- The user wants dimensional or unit consistency verification
- The user says "这个符号前面用的不一样" or "方程引用错了"
- `$paper-reviewer` or `$paper-logic` routes a notation issue here

## Do not use

- The user wants scientific logic review → use `$paper-logic`
- The user wants prose polish → use `$paper-writing`
- The user wants whole-paper triage → use `$paper-reviewer`
- The user wants figure/table presentation → use `$paper-visuals`

## Cross-references

- `$paper-reviewer` Tier-2/3 routes notation and symbol consistency issues to this skill
- `$paper-logic` routes notation issues discovered during logic audit to this skill
- Works alongside `$paper-writing` when notation fixes require surrounding prose adjustment

## Task ownership and boundaries

This skill owns:
- abbreviation first-use expansion enforcement
- symbol uniqueness and first-definition verification
- formula numbering continuity and cross-reference correctness
- pseudocode-formula variable alignment
- dimensional and unit consistency
- notation table ↔ body text reconciliation
- subscript/superscript and font style consistency
- theorem/lemma/definition environment symbol coherence

This skill does not own:
- whether the math is scientifically correct (→ `$paper-logic`)
- whether the prose around formulas reads well (→ `$paper-writing`)
- whether figures/tables are well-designed (→ `$paper-visuals`)

## Execution modes

### Quick scan (pre-submission 5-min check)

Run only these 3 critical checks:
- **A1** — abbreviation first-use expansion
- **S1** — symbol uniqueness
- **F2** — cross-reference accuracy

Use when time is tight or when the user asks for a fast pass.

### Full audit (carpet sweep)

Run all 24 checks below. Use for submission-ready manuscripts or when the user
asks for "地毯式审查" / "notation 全面检查".

## Required workflow

1. **Scope identification**:
   - Determine manuscript format (journal, conference, thesis)
   - Identify whether a notation/symbol table exists
   - Collect all abbreviations, symbols, and formulas from the manuscript
   - Choose execution mode (quick scan or full audit)

2. **Abbreviation audit** (A1–A6):
   First-use expansion, abstract independence, consistency, orphan expansions, standard vs custom, multilingual handling.

3. **Symbol audit** (S1–S7):
   Uniqueness, first-definition, font consistency, subscript/superscript, pseudocode alignment, notation table sync, theorem-environment coherence.

4. **Formula audit** (F1–F6):
   Numbering continuity, cross-reference accuracy, connector text, punctuation, multi-line alignment, inline vs display.

5. **Unit and dimension audit** (U1–U5):
   Unit presence, format consistency, dimensional consistency, magnitude plausibility, figure/table alignment.

   > For detailed check tables with pass criteria, see [references/audit-checklist.md](references/audit-checklist.md).
   > For notation conventions (font, punctuation, units), see [references/notation-conventions.md](references/notation-conventions.md).

6. **Deliver results**: output a `符号/缩写/公式审查清单` grouped by audit dimension, followed by a summary block.

## Output defaults

Use `符号/缩写/公式审查清单`:

| # | Dimension | Check | Location | Issue | Severity | Fix |
|---|---|---|---|---|---|---|
| 1 | Abbreviation | A1 | §3.1, L12 | "CNN" used without prior expansion | B | Add "Convolutional Neural Network (CNN)" at first use |
| ... | ... | ... | ... | ... | ... | ... |

Severity levels:
- **P0**: Causes reviewer confusion or misinterpretation (e.g., symbol collision)
- **A**: Professional standard violation (e.g., missing first-use expansion)
- **B**: Minor inconsistency (e.g., font style drift)
- **C**: Cosmetic preference (e.g., punctuation after equation)

> Severity definitions follow the shared paper-skill severity spec. See [`$paper-reviewer` references/severity-spec.md](../paper-reviewer/references/severity-spec.md).

### Summary block

Every audit output must end with:

```
## 审查概况
- 缩写 (Abbreviation): N issues (P0: x, A: y, B: z, C: w)
- 符号 (Symbol):        N issues (P0: x, A: y, B: z, C: w)
- 公式 (Formula):       N issues (P0: x, A: y, B: z, C: w)
- 单位 (Unit):          N issues (P0: x, A: y, B: z, C: w)
- 总计: N issues | 最高严重级: P0/A/B/C
- 执行模式: Quick scan / Full audit
```

## Hard constraints

- Do not change the mathematical content of any formula.
- Do not invent symbol definitions that the author did not provide; flag as missing instead.
- Report every instance found; do not summarize with "and others" or "etc."
- When the paper has a notation table, always cross-check it against body usage.
- Do not merge this audit with scientific logic review; route logic issues to `$paper-logic`.
- In quick scan mode, only report A1/S1/F2 issues; note that a full audit was not performed.
