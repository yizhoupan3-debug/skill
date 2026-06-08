# Review Dimensions

Use these as modes inside `paper-reviewer`, not as separate top-level owners.

Hard thresholds for **`audit_depth: exhaustive`** live in
[`../../paper-workbench/references/paper-exhaustive-audit.md`](../../paper-workbench/references/paper-exhaustive-audit.md)
— link, do not duplicate here.

- `logic mode`: claim ceiling, novelty, evidence coverage, ablation isolation, comparison fairness.
- `language / readability`: field-standard terms, terminology density, repetition, defensive tone, code/csv pointers, **EN slop / ZH 套话**, ladder/topic-sentence failures; normative detail in [`../../paper-workbench/references/research-language-norms.md`](../../paper-workbench/references/research-language-norms.md) + [`prose-quality-gate.md`](prose-quality-gate.md); finding handoff in [`../../paper-workbench/references/prose-chain-contract.md`](../../paper-workbench/references/prose-chain-contract.md); exhaustive sweep in `paper-exhaustive-audit.md` §Language.
- `notation sweep`: abbreviation first use, symbol uniqueness, equation numbering, cross-references, units (Pass2 under exhaustive math pass).
- `figure-table mode`: rendered readability, caption self-containment, axis/legend clarity, column mode; exhaustive visual pass in `paper-exhaustive-audit.md` §Visual.
- `length risk`: whether page pressure hides missing evidence or forces appendix routing.

Return verdict first. For **`audit_depth: compact`**, then the few blockers most likely to affect acceptance. For **exhaustive**, inherit full dimension findings from `paper-exhaustive-audit.md`.
