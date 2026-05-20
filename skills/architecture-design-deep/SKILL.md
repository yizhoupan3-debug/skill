---
name: architecture-design-deep
description: |
  Deep architectural and design review (review-only). Evaluates system design decisions, component boundaries, data flow, coupling, extensibility, and architectural risk.
  Does not rewrite implementation. Uses architecture-review as its countable deep-gate lane.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - $architecture-design-deep
  - architecture-design-deep
  - architecture review
  - design review
  - 架构评审
  - 设计评审
  - architecture design review
  - system design review
  - architectural review
  - 架构审查
  - 系统设计审查
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [architecture, design-review, code-review, delegation, adversarial-review]
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

# Architecture & Design Review (deep owner)

Judgment-focused review for system architecture and design decisions **without** rewriting by default. Portable across repositories: do **not** assume framework-specific files or audit commands exist unless the workspace is this skill/harness repo and the user's scope includes it.

## Default posture

- Assume a **hostile but fair** reviewer: maximize plausible failure under real abuse, scaling pressure, team evolution, dependency churn, or incomplete specification.
- **Analysis standard is unchanged**: still choose lenses internally, still exhaust findings **within each lens you selected**, still apply the severity evidence gate below. **Compact default output means less prose in chat, not shallower reasoning.**
- **Lens catalog, not a fixed runway**: choose lenses from [`references/design-lenses.md`](references/design-lenses.md). **Do not** treat every review as "must run every row." **Do** systematically exhaust findings **within each lens you selected**.
- When the user explicitly asks to **cover all dimensions** / **exhaust every lens** / **全维度**, apply the full catalog **and** use the **full report profile** (see Deliverable shape); evidence rules for P0/P1 stay the same.

## Compact envelope（硬性，宿主可见）

Rules for **everything the host/user sees** in chat under **default compact**—not for **internal** lens reasoning.

- **Severity line prefixes**: Except for a single-line **`Caveat:`** row (see below), **every** finding line **must** start with **`[P0]`**, **`[P1]`**, or **`[P2]`**. A **caveat / open question** may use **`[P2]`** plus a short parenthetical that evidence was downgraded **or** one line starting **`Caveat:`**—**equivalent** for "first finding line" and ordering (**P2 / caveat** bucket) below.
- **Prefix block (only before the first `[P0]` / `[P1]` / `[P2]` / `Caveat:`)**:
  - **With `Scope:`**: **Exactly one** line `Scope: …`. Optionally **one** more line **`Out of scope: …`** (single line only). The **very next** line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). **No** third prelude line, **no** tables, **no** headings between `Out of scope:` and that finding.
  - **Without `Scope:`**: The **first** host-visible line **must** be the first finding (`[P0]` / `[P1]` / `[P2]` / `Caveat:`). Do **not** use a standalone **`Out of scope:`** line ahead of findings—fold that note into the first finding or into your single `Scope:` line if you add one.
- **Forbidden before the first `[P*` / `Caveat:`** (other than the **`Scope:`** / optional **`Out of scope:`** lines above): Markdown **tables**; section headings whose role is **summary / 小结 / 分类 / 属于哪一类 / taxonomy** plus long prose; multi-sentence "scene setting." **Lens work stays implicit** unless the user asks for grouping, lens tables, or **full report profile**.
- **Verdict**: at most **one line**, **only after** the complete findings list in the same reply. Optional aggregate **`test/repro gap`** stays **≤ one line** after verdict—or folded into residual-risk—not as preamble.
- **Exception**: Only in **full report profile**—user explicitly asks for PR narrative, lens-by-lens tables, categorical summaries, **`Scope/Lenses/Omitted`**, audit-style sections, etc.

## Output profiles

### Default compact output (unless the user asks for narrative / lenses table / PR-style report)

- **Envelope**: Obey the **Compact envelope** section above.
- One list sorted **globally** as **P0 → P1 → P2 → caveat / open question** (within each level, rank by blast radius / confidence / affected surface).
- **Do not default** to a separate **Scope / Lenses / Omitted** block. **Prefix** rules: optional **one** line `Scope: …`; **if** you use `Scope:`, you may add **at most one** line `Out of scope: …`, then **immediately** the first **`[P*]` / `Caveat:`** line (see **Compact envelope**). **Without** `Scope:`, do **not** lead with standalone `Out of scope:`.
- **Verdict**: optional **at most one line** (`blocked | revise before merge | ship with caveats`), **after** findings **only** (never leading the reply).
- **Do not group findings by lens** in chat unless the user asks for grouping by lens or full audit trail.
- **Each finding**: one tight line plus optional indented evidence; minimal structure — **`[Pn] path:anchor`** — issue — impact — smallest verification or missing test (aligns with **Severity evidence gate**). **Caveat / open question** lines: prefer **`[P2]`** with downgrade note, or **`Caveat:`** as defined in **Compact envelope**—same evidence rules.

### Full report profile (explicit triggers only)

Use **only** when the user asks for **`Scope/Lenses/Omitted`**, **lens-by-lens sections**, **PR / 述职叙事**, categorical deliverables (**类型 + 说明** matrices), **`属于哪一类` taxonomy**, **Markdown summary tables** as the artifact, **exhaust every lens**, **audit-style report**, or other **explicit narrative**. Vague 「有什么问题」「全面 review」**alone** stays **compact**—do **not** treat them as opting into this profile.

Then you may use a preamble (**Scope**, **Lenses**, **Omitted**), **`verdict`**, findings **grouped by lens**, then **`test / repro gap`**, optional **`external calibration`**, **`next move`**—same rigor, richer packaging.

## Lane contracts

For broad/deep/architecture-level design review, the minimum compliant default is **one** independent read-only reviewer subagent with explicit JSON boolean **`fork_context=false`** before main-thread synthesis. Admit a **second** reviewer lane only when scope is broad enough, shared context is low enough, and the expected information gain clearly outweighs synchronization/token cost. Use **three or more** lanes only for explicit full-repo, multi-module, or high-risk review asks. **Narrow single-component** review may stay on the main thread (no multi-lane requirement) unless the user asks for deep/adversarial coverage or explicitly authorizes multi-lane review. When additional subagents are admitted, keep them read-only and **artifact-disjoint**. Split subagents by **your selected lenses**, not by a hard-coded global list. Do **not** have multiple lanes silently edit shared files mid-review.

**Host countable evidence (Cursor / Codex `REVIEW_GATE` / Codex Stop ledger)** matches `hook_common::is_deep_review_gate_lane_normalized`: lane must be in `review_gate.deep_gate_lanes` only (`general-purpose` / `best-of-n-runner` and normalized equivalents — see `docs/host_adapter_contract.md` §0.1). **`architecture-review` is not a registered countable lane** unless added to that array. **`explore`, custom lane names, and Claude-only `review*` spellings do not count** on Cursor/Codex. **Claude Code** uses `review_gate.claude_reviewer_lanes`.

Lane outputs must cite **locations** (paths + anchors / symbols where possible).

**Framework-repo optional evidence** (only when this workspace is this harness/skill framework repository and scope touches it): you may cite local checklists or `router-rs framework maint` audit-style commands as **read-only** evidence—never as a dependency for reviews of other codebases.

## External / network research lane (optional but recommended)

Use only when the user allows network/tools or the scope touches third-party services, frameworks, or architectural patterns outside the team's direct experience. When marking work "deep external," prefer the **full report profile** for the calibrated section.

**If you stay in default compact** (user did **not** opt into **full report profile**): do **not** place **Claims / Contradiction / Unknowns / Retrieval_trace** (or RFV §A–B **headings**) **before** the first **`[P0]` / `[P1]` / `[P2]` / `Caveat:`** line. After the findings list begins, external material **may** appear only as **(a)** indented bullets **under** the specific **`[P*]` / `Caveat:`** line they support, or **(b)** plain continuation (no new H1/H2) **immediately after the last finding line** and **before** the optional **one-line** `verdict`—still **no** standalone "Claims / Contradiction …" **section headers** and **no** Markdown tables in that gap.

When marking work "deep external" **and** the user accepts **full report profile**, you may use the heading block in the preamble per that profile.

### External checklist (full report template only)

The following bullets apply **only** in **full report profile** (or an explicit preamble the user requested for external calibration)—**not** as a default tail to paste after compact findings:

- Produce **Claims** backed by citations (changelog URL, GitHub Advisory ID, CVE, release notes DOI/issue, published architectural decision records).
- **Contradiction sweep**: cite evidence that contradicts or limits each high-confidence Claim.
- **Unknowns**: what still cannot be asserted from reachable evidence alone.
- **Retrieval_trace** (minimal): queries / sources scanned, inclusion/exclusion heuristic, stale assumptions rejected.

Structured output expectations align with [`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B (same headings whenever you mark work as "deep external," even outside an RFV ledger).

## Severity evidence gate

- **P0/P1 requires evidence**: include at least one of a concrete call chain, a design-level repro path, a checked test gap, or a cited external advisory/source. Without that, downgrade to P2, caveat, or open question.
- **No hollow findings**: every finding must include path + symbol/line anchor, architectural impact (scalability, maintainability, correctness), and the smallest verification that would confirm it.
- **Testing honesty**: if tests were not run, say so compactly once (footer of findings or residual-risk line) and name the residual risk.
- **Design claims**: state the architectural constraint being violated; speculation without a concrete design principle or invariant is a caveat/open question, not a blocker.

## Deliverable shape

**Default (compact)** — **top to bottom** for host-visible text:

1. **Optional prefix** (see **Compact envelope**): **zero to two** lines only—**`Scope:`** (optional), then optionally **one** **`Out of scope:`** line **only if** you already used `Scope:`. **No** other lines before findings.
2. **`Findings`**: single list, severity order **P0 → P1 → P2 → caveats**, each item evidence-gated as above; the first **`[P*` / `Caveat:`** line must come **immediately after** the prefix (no tables, no headings in between).
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
- If the artifact is screenshots or rendered UI decks, `$visual-review` complements but does not replace architecture/design lanes.
- If the user needs **code-level review** rather than architecture/design review, prefer `code-review-deep` as the narrower owner when routing applies.
- If the user needs **paper/manuscript** judgment or **GitHub PR comment triage** as the primary task, prefer the narrower owners (`paper-workbench`, `gh-address-comments`, etc.).
