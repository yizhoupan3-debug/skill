# Paper Lanes

`paper-workbench` is the manuscript-level front door.

Lane choices:

- `whole manuscript`: submission readiness, venue fit, and top blockers.
- `review-only`: hand to `paper-reviewer` when the user explicitly wants critique without edits.
- `revision`: hand to `paper-reviser` when comments/findings are known and edits should happen now.
- `bounded prose`: hand to `paper-writing` when claim scope is fixed and only wording should change.
- `literature`: keep ref / related-work corpus building under `paper-workbench` as source-backed context, then hand bounded prose to `paper-writing` (no separate top-level literature skill).

Dimension modes:

- `logic mode`: claim, novelty, evidence, ablation, and experiment defensibility.
- `notation sweep`: abbreviations, symbols, equations, references, and units.
- `length budget mode`: page/word budget, cuts, appendix routing, and expansion plan.
- `figure-table mode`: figures, tables, captions, legends, axes, density, and layout.
