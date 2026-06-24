# Paper Exhaustive Audit Contract

Single source of truth for **`audit_depth: exhaustive`** manuscript review.
`$paper-workbench` and `$paper-reviewer` **inherit this file** — do not duplicate
steps, checklists, or output envelopes in SKILL bodies.

Normative cross-refs (do not redefine here):

- Claim / evidence / R&R: [`claim-evidence-ladder.md`](claim-evidence-ladder.md)
- Language / tone / terminology: [`research-language-norms.md`](research-language-norms.md)
- Severity labels: [`severity-spec.md`](severity-spec.md)
- Optional rubric matrix: [`rubric-audit-bridge.md`](rubric-audit-bridge.md)

## Persona

- **Hostile but fair** top-tier reviewer posture; find real problems, not comfort.
- **No identity preamble** at the start of the reply (do not restate role).
- **No fabricated issues** — every finding must cite manuscript location + visible evidence.
- If modification advice is given, it must be **root-cause repair**, not patches or tone-only hedges.

## When to use

- `$paper-workbench` routed with **`audit_depth: exhaustive`** (default for whole-paper
  readiness asks such as 能不能投 / 整篇严审 / 投稿前把关).
- User token `audit_depth: exhaustive` on its own line.
- User explicitly asks for 穷举 / 逐句 / 逐公式 / 全文审核 depth.

## When not to use

- **`audit_depth: compact`** or narrow single-dimension asks → use compressed reviewer workflow.
- Filesystem-backed G0–G14 protocol artifacts → [`paper-gate-protocol.md`](paper-gate-protocol.md)
  (internal machinery; not the default user-facing exhaustive UI).

## Execution order

After locking target bar and claim map:

1. **Strategic pass** (below)
2. **Visual pass** (below)
3. **Math pass** — Pass1 overall → Pass2 symbol/formula sweep (below)
4. **Language pass** — Pass1 structure → Pass2 sentence sweep (below)
5. **Rubric matrix** (optional) — if user supplied rubric / assignment / Bonus criteria
6. **Merge** severity + `Warning` items; emit output envelope

Do **not** emit G0–G14 gate progress as the main deliverable. Verdict first, then
dimension-grouped **findings** (not a gate state machine report).

---

## Strategic (claim / support / narrative)

### External calibration (required unless skipped)

Default: run external calibration when network is available.

- Closest prior work, venue/article-type norms, required baselines, citation currency.
- Follow [docs/architecture.md](docs/architecture.md)
  §A–B: **Claims**, **Contradiction sweep**, **Unknowns**, **retrieval_trace**.

If skipped, output **`skip_reason`** (e.g. offline, user waived, no network) — do not
pretend calibration ran.

### Support

- Map each surviving claim to **decisive evidence anchors** (figure/table/theorem/experiment).
- Flag claim–evidence gaps per [`claim-evidence-ladder.md`](claim-evidence-ladder.md).
- When B-tier blockers exist, populate **`evidence_first_options`** in the output envelope
  (same ladder semantics — do not emit a second competing repair list).

### Narrative

- Story must **orbit the central claim** with rigorous evidence flow.
- Reject audit-style taxonomy dumps; findings are grouped by dimension, not gate id.
- Flag paragraphs that are pure setup without claim linkage or evidence payoff.

---

## Visual (figures / tables / layout)

### Rendered artifacts (required when available)

- Prefer PDF pages, exported figures, or screenshots → route **`$visual-review`**
  (figure-table / page professionalism / table readability lenses).
- If no render exists: mark visual findings **`indeterminate`** and list required artifacts
  (PDF export, figure PNGs at submission scale). **Do not invent layout defects.**

### Compile witness (read-only, when LaTeX source + log exist)

Scan build log (e.g. `*.log`) for at least:

- `Overfull \hbox` / `\vbox` (layout stress)
- `Citation ... undefined` / `Reference ... undefined`
- Column / float warnings relevant to two-column mode

Do not modify toolchain; witness only.

### Figure / table bar

- **Sufficient and necessary** — each figure/table earns its place; remove decorative panels.
- **Caption**: extremely short, self-contained (variables, cohort/data, one takeaway).
- **No figure notes** stacking explanations that belong in caption or main text.
- **Tables**: highly digitized cells — minimal prose in cells; stats/headers explicit.
- **Column mode**: check **document class / single vs double column / figure width**
  against legibility at final scale (not source-file preview alone).

---

## Math (logic / notation / proofs)

### Pass1 — overall closure

- Theorem/lemma chain supports surviving claims; assumptions explicit.
- No decorative math (**overmath**) — see G4 spirit in
  [`review-rubric-playbook.md`](review-rubric-playbook.md).
- Proof placement: main text vs appendix serves narrative rhythm (not evasion).

### Pass2 — symbol / formula sweep

- **Every symbol defined before first use**; global consistency across sections.
- **Each main-text equation / theorem**: tag derivation as **sufficient**, **necessary**,
  **both**, or **neither (overmath / gap)** with location.
- If the manuscript contains formal theorem blocks: **mandatory** read-only witness via
  **`$math-derivation`** for proof gaps (does not replace paper-level logic mode).

---

## Language (structure / prose / citations)

Normative detail: [`research-language-norms.md`](research-language-norms.md) + **prose chain**
[`prose-quality-gate.md`](prose-quality-gate.md).
Handoff shape: [`prose-chain-contract.md`](prose-chain-contract.md) §审稿→写作.
This section is the **audit checklist** only.

### Pass1 — structure (Section / Subsection / Paragraph)

- For each main-text block: keep / cut / merge / split / move to appendix?
- Remove audit-report filler; ensure IMRaD (or venue) spine serves the claim.

### Pass2 — sentence sweep

- Infer or record **`language_register`** per main-text block (`en_submission` / `zh_manuscript` / `mixed`).
- Direct, field-standard wording; **no defensive / AI-slop / invented jargon** (norms §1–3 + prose-quality-gate slop lists).
- Each **language** finding **must** include `prose_repair_class` + `register` (see prose-chain-contract).
- Abbreviations expanded on first use.
- **Zero tolerance** for low-level grammar/spelling in submission-facing text.
- **Citation precision** — claims tied to correct sources.
- **Fail**: three or more consecutive citations in one spot (citation cluster dump).

---

## Rubric (optional)

When the user supplies assignment text, rubric, or **Bonus** criteria, run
[`rubric-audit-bridge.md`](rubric-audit-bridge.md) before final merge.

---

## Severity and Warning

Use [`severity-spec.md`](severity-spec.md):

- **P0 / A / B / C** as today.
- **`Warning`**: subtle omission, unstated boundary, or likely reader misread — must list
  in exhaustive mode; may co-tag with B (`B + Warning`).

**Exhaustive mode**: do **not** truncate to "top 3" or "top blockers". List **all**
material findings by dimension, severity-sorted within each dimension (P0/A first).

---

## Output envelope (exhaustive)

```text
verdict: 可投 | 大修后再投 | 不建议投 | 需要补关键证据
audit_depth: exhaustive
requirement_matrix: (optional — see rubric-audit-bridge)
findings_by_dimension:
  strategic: [{id, severity, location, issue, evidence}]
  visual: [...]
  math: [...]
  language: [...]
warning_items: [{id, location, issue, evidence}]
external_calibration: (summary when used)
skip_reason: (when external calibration skipped)
evidence_first_options: (when B-tier blockers — per claim-evidence-ladder)
next_honest_move:
```

**Compact mode** (`audit_depth: compact`) keeps verdict + top blockers summary only;
do not use this envelope unless user escalates to exhaustive.
