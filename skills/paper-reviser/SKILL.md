---
name: paper-reviser
description: |
  Specialist revision lane behind `$paper-workbench`. Use when the route is
  already clearly "change the paper now" based on reviewer comments, known
  findings, or a fixed decision to narrow scope. This skill may repair, narrow,
  delete, de-emphasize, or move material to the appendix when that is the
  honest fix. For 顶刊/顶会/top-tier revision, it turns acceptance blockers into
  manuscript changes only after the scientific claim boundary is known.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - $paper-reviser
  - paper-reviser
  - 只进改稿 lane
  - 按现有 findings 直接改稿
  - 直接改稿不要先审
  - 缩口径
  - 按这个维度改
  - 只改摘要
  - 只改图表维度
  - 写 rebuttal
  - 顶刊标准改稿
  - 顶会标准改稿
  - 顶刊顶会改稿
  - top-tier revision
  - revise for top conference
  - revise for top journal
  - 精准修改
  - 大面积重构
  - "edit_scope: surgical"
  - "edit_scope: refactor"
metadata:
  version: "3.6.0"
  platforms: [codex]
  tags: [paper, manuscript, revise, reviewer-comments, rebuttal, appendix-routing]
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

This skill is the revision specialist lane behind `$paper-workbench`.

It owns the paper-facing execution step: after the problems are known, actually
change the manuscript in the most honest direction.

The execution model is:

- main revision chain = serial
- local specialist checks and cleanup = bounded parallel sidecars
- merge-back and final accept/reject of edits = local

## Edit scope gate

Honor **`edit_scope`** from
[`../paper-workbench/references/edit-scope-gate.md`](../paper-workbench/references/edit-scope-gate.md)
before applying edits:

- **`surgical`**: execute only the listed reviewer items / blockers / slices; do
  not expand into whole-paper restructuring, unsolicited appendix routing, or
  cross-section narrative rewrites.
- **`refactor`**: full allowed-edit decisions (repair, narrow, delete, appendix,
  de-emphasize) across the manuscript as needed.

If the user did not declare scope, default to **`surgical`** until clarified.

Under **`surgical`**, obey the **硬等级 / 防扩写 / 整篇回贴禁令 / 静默全局替换 / 锚定 / 改动上限 / 默认交付形态 / 交付清单 / 自检** contract in
[`../paper-workbench/references/edit-scope-gate.md`](../paper-workbench/references/edit-scope-gate.md)
— especially: no edits outside listed **`scope_items`**, no 「顺手」polish on neighbor
paragraphs, no **global terminology or punctuation harmonization** without explicit scope rows, **单锚多改**必须通过多条 `scope_item` 或升格 `refactor`, and enumerated `change_id` + `original_excerpt` accountability. **禁止**在用户只授权局部时交出「润色后的整节/整稿」作为主要输出。

## Use this when

- The user explicitly wants edits now, not the front door
- The task is driven by reviewer comments, a review checklist, or a known blocker
- The route is already clearly revise-only
- The paper needs claim downgrade, appendix routing, de-emphasis, or deletion instead of forced repair
- The user wants rebuttal or response-letter work tied to real manuscript edits
- The user has a top-tier readiness finding and wants concrete manuscript
  changes against that blocker

## Do not use

- The user wants one front door for the paper task -> use `$paper-workbench`
- The user is still asking "能不能投" or wants the first review pass -> use `$paper-reviewer`
- The user wants only local wording polish with fixed scientific scope -> use `$paper-writing`
- The user wants only science-level critique without edits -> use `$paper-reviewer` logic mode

## User-facing modes

Use one of only two external modes:

- `按审稿意见改`: default when the user generally wants the manuscript fixed
- `只改这一维`: only when the user explicitly names one dimension or one block

Do not make the user speak in gate language unless they already are.

## Allowed edit decisions

**Evidence-before-narrow default**: when findings say the blocker is closable with
additional experiments, analysis, baselines, or decisive figures/tables, the
primary batch is **`repair`** — follow
[`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md).
Use **`narrow` / `de-emphasize` / `delete`** as the main move only when that doc's
primary-narrowing conditions apply, or the user explicitly chooses narrowing over
new evidence.

When the strongest honest path is not "repair everything", this skill may:

- repair
- narrow
- delete
- move to appendix
- de-emphasize
- disclose as limitation

These are not edge cases. They are part of the normal contract.

## What this skill should deliver

**Before describing edits**，先做 **`tone_audit`**（四句 checklist，对齐
[`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
**§3**，与 `$paper-writing` Output Defaults 同一映射：(a) 内部口径、(b) 防御口径、(c)
负面对比骨架、(d) `but` / `not` / `rather than` 堆叠。**若本 batch 仅改结构、图表管线或排版而未触及中英文句子**，不写四项检视，改为显式一行 **`本 batch 未触达用语层`** 并说明范围（例如仅 Fig.3 重排 / 附录搬家）。

Default output should stay simple:

1. what was changed in this slice
2. whether the blocker is resolved, partially resolved, or still blocked
3. whether the next step is more revision, re-review, or new evidence

Default user-facing wording contract:

- Prefer author-facing language: `revision done`, `remaining blocker`,
  `next rewrite target`.
- Keep protocol terms internal by default: `gate`, `backjump`, `lane`,
  `manifest`.
- Surface protocol terms only when the user asks for protocol artifacts.

For 顶刊/顶会/top-tier revision, each edit batch should also name which selective
venue risk it reduces: contribution clarity, closest-work separation, decisive
evidence, claim ceiling, reproducibility, figure/table persuasiveness, or
front-door story.

For multi-round revision, each batch must also report:

```text
claim_ledger_delta:
evidence_anchor_delta:
drift_check_result:
```

If the user is running the protocol-backed workflow, follow
[`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md). Treat the protocol as
internal state management, not as the main user interface.

In protocol mode, do not rewrite the whole paper in one undifferentiated pass.
Keep the active blocker serial, and use sidecar lanes only for bounded slices
such as citation fixes, figure/table cleanup, notation audit, mirror cleanup, or
local prose edits after the claim boundary is frozen.

If you need to materialize a parallel batch on disk, follow the lane manifest
contract in [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md). Do not
assume a scaffold script exists.

## 审稿意见驱动时的硬契约（默认开启）

当输入含 **程序化审稿条目、Major/Minor 列表或 point-by-point 要求**：

- **每条须有关停物**：手稿 diff/hunk、`evidence_anchor_delta`、新增的图/表/附录编号，或显式 **`cannot_fix_because`**（硬阻塞）；禁止用「重写 limitation」「语气变软」「仅改摘要」作为主回应去吞 **repair 类** 意见。
- **禁止逃避默认包**：**(a)** 静默降 claim 而无 [`claim-evidence-ladder`](../paper-workbench/references/claim-evidence-ladder.md) 许可理由；**(b)** Defense prose 加长代替补实验/对照/澄清；**(c)** 只做 rebuttal 字稿不交对手稿可追溯改动。
- **`surgical`**：仍遵守防扩写，但「精准」≠「只糊弄文字」——若某条意见要求改图注、方法段或一张表，必须把该条写入 **`scope_items`** 并完成对应表面修改；不得借 surgical 只做 Abstract hedge。
- **`refactor`**：允许大范围按意见重组，但若 findings 仍为 B 类可补，主批次仍以 **repair** 为主轴；narrow/delete/appendix 组合拳须有阶梯上的 **`narrowing_is_primary_because`**，不得只因改稿省事就缩口径。
- **代码/实现类 point**：默认产出 **方法段或补充材料中的可执行复现说明** + **算法—源码对照**（或 tag 化 bundle），并修任何被证伪的复杂度/默认超参/确定性叙述；**禁止**把关停写成「将公开代码」单句 rebuttal。
- **数学/推导类 point**：默认产出 **附录补证、定理勘误、或收窄后的正式陈述**（含公式编号变更说明）；**禁止**仅改直观 English 或把失败证明悄悄改成散文；若 `$math-derivation` 或论文逻辑门已参与，保持与 `claim_ledger` 一致。

## Internal routing notes

- Use `logic repair` mode when a revision depends on claim-vs-evidence repair
- Use `$citation-management` for citation support changes
- Use `$paper-writing` for local prose rewriting after the claim boundary is fixed
- Use [`../paper-workbench/references/top-tier-paper-standard.md`](../paper-workbench/references/top-tier-paper-standard.md)
  as the acceptance-risk checklist when the user wants top-tier revision

For revision dimension modes, use
[`references/revision-modes.md`](references/revision-modes.md).

For rebuttal letter / point-by-point response patterns (the canonical templates
live under `paper-writing` because the prose itself is local-rewrite-shaped),
use
[`../paper-writing/references/rebuttal-patterns.md`](../paper-writing/references/rebuttal-patterns.md)
when the rebuttal must be tied to manuscript edits owned here.
- Use `figure-table repair`, `$visual-review`, and `$pdf` for final figure, table, or layout changes
- When multiple local cleanup surfaces are independent, run them as bounded sidecar lanes and merge locally before closing the gate

## Hard rules

- Honor [`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)
  when edits touch naming, metrics, or repeated arguments; do not introduce
  undefined compound coinages while repairing claims unless the decision lane
  explicitly allows new definitional material; when stripping internal phrasing,
  replace code identifiers and bare `.csv`/path result pointers with prose +
  proper tables/figures or supplement numbering per that doc
- Do not hide evidence that breaks a claim the paper still keeps
- Do not use appendix moves as a substitute for an honest claim downgrade
- Do not parallelize multiple gate-closing decisions at once
- Do not expand a one-slice edit request into a full-paper rewrite
- Under **`surgical`**, do not substitute **whole-file rewritten text** for
  **`scope_items`-bounded work** unless the user explicitly escalates to
  **`refactor`**; obey the gate doc's patch-first delivery rule
- If a blocker needs new experiments, say so instead of polishing around it
- Do not substitute **claim downgrade** or limitation prose for **runnable**
  experiments or analyses when findings still classify the gap as closable that
  way; see
  [`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md)
- Do not edit prose that changes claim level unless the claim decision lane
  explicitly approves and records the claim ledger delta
- When reviewer comments are in scope: do not ship a revision round whose main
  effect is softer claims + longer caveats while leaving the flagged evidence /
  comparison / protocol gap untouched; honor
  [`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md)
  §「审稿意见 / R&R：逐条关停与逃逸红线」及 **§代码/实现质疑**、**§数学/推导质疑**
- Code- or math-skeptic reviewer points are **not prose-only lanes**: closing a
  round must include **checksumable artifacts** (proof deltas, reproducibility
  anchors, corrected statements)—not vague release promises or informal rewrites
  that leave the substantive doubt untouched
