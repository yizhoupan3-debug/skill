---
name: code-review-deep
description: |
  Deep adversarial-style code review (review-only). Default visible output is a compact, severity-sorted findings list; narrative sections only when explicitly requested.
  Model selects lenses from an extensible catalog (core + optional: first principles/subtraction, dead-code signals, stale docs); exhaustive within chosen lenses.
  Broad/deep/PR-level work authorizes read-only independent reviewer subagents (fork_context=false) before main-thread synthesis. Does not silently rewrite implementation
  unless the user explicitly exits review-only posture.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - $code-review-deep
  - code-review-deep
  - review
  - code review
  - 代码审查
  - 帮我 review
  - deep code review
  - 深度 code review
  - 深度代码审查
  - 严苛代码评审
  - security code review
  - security-focused code review
  - threat model review
  - adversarial code review
  - 只允许审不改
  - review-only 代码审查
  - CVE 审查
  - dependency audit PR
  - supply chain review
  - 供应链安全
metadata:
  version: "1.2.3"
  platforms: [supported]
  tags: [code-review, security, correctness, delegation, adversarial-review]
framework_roles:
  - detector
  - planner
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local
---

# Code review (deep owner)

Judgment-focused review for code and change sets **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo and the user’s scope includes it.

## Default posture

- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, regressions,
  flaky ops, closest prior API expectations, dependency churn, or incomplete tests.
- **Analysis standard is unchanged**: still choose lenses internally, still exhaust findings **within each lens you selected**, still apply the severity evidence gate below. **Compact default output means less prose in chat, not shallower reasoning.**
- **Lens catalog, not a fixed runway**: choose lenses from [`references/review-dimensions.md`](references/review-dimensions.md). **Do not** treat every review as “must run every row.” **Do** systematically exhaust findings **within each lens you selected**.
- When the user explicitly asks to **cover all dimensions** / **exhaust every lens** / **全维度**, apply the full catalog **and** use the **full report profile** (see Deliverable shape); evidence rules for P0/P1 stay the same.

## Compact envelope（硬性，宿主可见）

Rules for **everything the host/user sees** in chat under **default compact**—not for **internal** lens reasoning.

- **Severity line prefixes**: Except for a single-line **`Caveat:`** row (see below), **every** finding line **must** start with **`[P0]`**, **`[P1]`**, or **`[P2]`**. A **caveat / open question** may use **`[P2]`** plus a short parenthetical that evidence was downgraded **or** one line starting **`Caveat:`**—**equivalent** for “first finding line” and ordering (**P2 / caveat** bucket) below.
- **Prefix block (only before the first `[P0]` / `[P1]` / `[P2]` / `Caveat:`)**:
  - **With `Scope:`**: **Exactly one** line `Scope: …`. Optionally **one** more line **`Out of scope: …`** (single line only). The **very next** line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). **No** third prelude line, **no** tables, **no** “小结 / 分类 / 属于哪一类” headings between `Out of scope:` and that finding.
  - **Without `Scope:`**: The **first** host-visible line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). Do **not** use a standalone **`Out of scope:`** line ahead of findings—fold that note into the first finding or into your single `Scope:` line if you add one.
- **Forbidden before the first `[P*` / `Caveat:`** (other than the **`Scope:`** / optional **`Out of scope:`** lines above): Markdown **tables**; section headings whose role is **summary / 小结 / 分类 / 属于哪一类 / taxonomy** plus long prose; multi-sentence “scene setting.” **Lens work stays implicit** unless the user asks for grouping, lens tables, or **full report profile**.
- **Verdict**: at most **one line**, **only after** the complete findings list in the same reply. Optional aggregate **`test/repro gap`** stays **≤ one line** after verdict—or folded into residual-risk—not as preamble.
- **Exception**: Only in **full report profile**—user explicitly asks for PR narrative, lens-by-lens tables, categorical summaries, **`Scope/Lenses/Omitted`**, audit-style sections, etc.

## Output profiles

### Default compact output (unless the user asks for narrative / lenses table / PR-style report)

- **Envelope**: Obey the **Compact envelope** section above.
- One list sorted **globally** as **P0 → P1 → P2 → caveat / open question** (within each level, rank by blast radius / confidence / affected surface).
- **Do not default** to a separate **Scope / Lenses / Omitted** block. **Prefix** rules: optional **one** line `Scope: …`; **if** you use `Scope:`, you may add **at most one** line `Out of scope: …`, then **immediately** the first **`[P*]` / `Caveat:`** line (see **Compact envelope**). **Without** `Scope:`, do **not** lead with standalone `Out of scope:`.
- **Verdict**: optional **at most one line** (`blocked | revise before merge | ship with caveats`), **after** findings **only** (never leading the reply).
- **Do not group findings by lens** in chat unless the user asks for grouping by lens or full audit trail.
- **Each finding**: one tight line plus optional indented evidence; minimal structure —
  **`[Pn] path:anchor`** — issue — impact / exploitability — smallest verification or missing test (aligns with **Severity evidence gate**). **Caveat / open question** lines: prefer **`[P2]`** with downgrade note, or **`Caveat:`** as defined in **Compact envelope**—same evidence rules.

### Full report profile (explicit triggers only)

Use **only** when the user asks for **`Scope/Lenses/Omitted`**, **lens-by-lens sections**, **PR / 述职叙事**, categorical deliverables (**类型 + 说明** matrices), **`属于哪一类` taxonomy**, **Markdown summary tables** as the artifact, **exhaust every lens**, **audit-style report**, or other **explicit narrative**. Vague 「有什么问题」「全面review」**alone** stays **compact**—do **not** treat them as opting into this profile.

Then you may use a preamble (**Scope**, **Lenses**, **Omitted**), **`verdict`**, findings **grouped by lens**, then **`test / repro gap`**, optional **`external calibration`**, **`next move`**—same rigor, richer packaging.

## Lane contracts

For broad/deep/PR-level code review, the default is **at least two** parallel read-only reviewer subagents, each with explicit JSON boolean **`fork_context=false`**, split by **disjoint lens bundles**, before main-thread synthesis. When the user’s prompt hits **parallel breadth** signals (review + breadth + scope together), prefer **≥3** parallel lanes—same read-only / artifact-disjoint rules (**this paragraph is the operational detail**; Cursor `beforeSubmit` only injects a **one-line** pointer to this file, not a long checklist). **Narrow single-file / single-hunk** review may stay on the main thread (no multi-lane requirement) unless the user asks for deep/adversarial coverage or explicitly authorizes multi-lane review. When additional subagents are admitted, keep them read-only and **artifact-disjoint**. Split subagents by **your selected lenses**, not by a hard-coded global list. Do **not** have multiple lanes silently edit shared files mid-review.

**Host countable evidence (Cursor / Codex `REVIEW_GATE` / Codex Stop ledger)** matches `hook_common::is_deep_review_gate_lane_normalized`: the subagent lane (after host normalization) must be **`general-purpose`** or **`best-of-n-runner`** (including spellings like `generalpurpose` / `bestofnrunner`). **`explore`, `ci-investigator`, `cursor-guide`, custom lane names, etc. do not count** toward clearing the independent-reviewer gate—even with **`fork_context=false`**. **Claude Code** accepts additional `review*` lane spellings; do not assume those satisfy Cursor/Codex hooks.

Lane outputs must cite **locations** (paths + anchors / symbols where possible).

**Framework-repo optional evidence** (only when this workspace is this harness/skill framework repository and scope touches it): you may cite local checklists or `router-rs framework maint` audit-style commands as **read-only** evidence—never as a dependency for reviews of other codebases.

## External / network research lane (optional but recommended)

Use only when the user allows network/tools or the scope touches third-party crates/services or known vulnerability classes. When marking work “deep external,” prefer the **full report profile** for the calibrated section.

**If you stay in default compact** (user did **not** opt into **full report profile**): do **not** place **Claims / Contradiction / Unknowns / Retrieval_trace** (or RFV §A–B **headings**) **before** the first **`[P0]` / `[P1]` / `[P2]` / `Caveat:`** line. After the findings list begins, external material **may** appear only as **(a)** indented bullets **under** the specific **`[P*]` / `Caveat:`** line they support, or **(b)** plain continuation (no new H1/H2) **immediately after the last finding line** and **before** the optional **one-line** `verdict`—still **no** standalone “Claims / Contradiction …” **section headers** and **no** Markdown tables in that gap. **Do not** insert a four-part **Claims / Contradiction / Unknowns / Retrieval** chapter between findings and `verdict` unless the user has opted into **full report profile**.

When marking work “deep external” **and** the user accepts **full report profile**, you may use the heading block in the preamble per that profile.

### External checklist (full report template only)

The following bullets apply **only** in **full report profile** (or an explicit preamble the user requested for external calibration)—**not** as a default tail to paste after compact findings:

- Produce **Claims** backed by citations (changelog URL, GitHub Advisory ID, CVE, release notes DOI/issue).
- **Contradiction sweep**: cite evidence that contradicts or limits each high-confidence Claim.
- **Unknowns**: what still cannot be asserted from reachable evidence alone.
- **Retrieval_trace** (minimal): queries / sources scanned, inclusion/exclusion heuristic, stale assumptions rejected.

Structured output expectations align with
[`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B (same headings whenever you mark work as “deep external,” even outside an RFV ledger).

## Severity evidence gate

- **P0/P1 requires evidence**: include at least one of a concrete call chain, a repro path, a checked test gap, or a cited external advisory/source. Without that, downgrade to P2, caveat, or open question.
- **No hollow findings**: every finding must include path + symbol/line anchor, user or operational impact, and the smallest verification or missing test that would confirm it.
- **Testing honesty**: if tests were not run, say so compactly once (footer of findings or residual-risk line) and name the residual risk.
- **Security claims**: state exploitability or blast radius; speculative abuse without a reachable path is a caveat/open question, not a blocker.

## Deliverable shape

**Default (compact)** — **top to bottom** for host-visible text:

1. **Optional prefix** (see **Compact envelope**): **zero to two** lines only—**`Scope:`** (optional), then optionally **one** **`Out of scope:`** line **only if** you already used `Scope:`. **No** other lines before findings.
2. **`Findings`**: single list, severity order **P0 → P1 → P2 → caveats**, each item evidence-gated as above; the first **`[P*` / `Caveat:`** line must come **immediately after** the prefix (no tables, no “小结/分类” sections in between).
3. Optional **one-line** `verdict` **after** that list.
4. Optional **one-line** `test/repro gap`; omit if each finding already carries verification.

**Full report profile** — explicit triggers only (see **Output profiles**):

0. Scope / Lenses / Omitted (or equivalent narrative opener when user asked for taxonomy).
1. `verdict` (one line).
2. Findings grouped by applied lens with P0–P2 tags.
3. `test / repro gap`.
4. `external calibration` (if external lane used).
5. `next move` (implementer handoff).

## Integration / boundaries

- If the task is repo closeout Git operations, `$gitx` still owns staging history; reuse this lane for substantive diff critique only.
- If the artifact is screenshots or rendered UI decks, `$visual-review` complements but does not replace correctness/security lanes.
- If the user needs **paper/manuscript** judgment or **GitHub PR comment triage** as the primary task, prefer the narrower owners (`paper-workbench`, `gh-address-comments`, etc.) when routing applies.
