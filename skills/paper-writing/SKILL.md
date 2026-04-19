---
name: paper-writing
description: |
  Polish already-decided academic paper prose without changing evidence or
  claim boundaries. Use for section- or paragraph-level rewriting of
  abstracts, introductions, related work, captions, conclusions, and rebuttal
  wording after the review or gate decision is already known.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 润色摘要
  - 改摘要表达
  - 改引言表达
  - 精修 introduction
  - 改 conclusion 表述
  - 改相关工作文字
  - 改 caption
  - caption polish
  - rebuttal wording
  - 回复信润色
  - 文字精修
  - prose polish
  - rewrite abstract
  - rewrite introduction
  - polish conclusion
  - rewrite captions
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags: [paper, writing, rewrite, rebuttal, abstract, prose, academic]
framework_roles:
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: false
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: low
source: local
---

- **Dual-Dimension Audit (Pre: Outline-Logic/Tone, Post: Argument-Fidelity/Citation-Accuracy Results)** → `$execution-audit-codex` [Overlay]

# Paper Writing

This skill owns **local paper prose revision after scope is already fixed**.

## Finding-driven framework compatibility

When this skill is used in a revision flow, it should consume findings from
upstream review skills or reviewer comments normalized into findings, then
return revised prose with explicit status on the addressed items.

Minimum compatibility expectations:
- preserve incoming `finding_id` values when present
- keep `severity_native` and other paper-native labels intact
- record whether the item was resolved, partially resolved, or blocked
- emit verification status for the revised prose instead of re-litigating the
  original issue list

When this skill is invoked from the paper gate chain, also treat the markdown
packet as the source of truth:

- read the active gate file first
- respect `Frozen Inputs`
- do not rely on remembered chat prose when the gate markdown already defines the live claim surface

## When to use

- The user wants wording review or rewriting after the scientific scope is already decided
- The target is abstract, intro, captions, related work, conclusion, rebuttal, or response text
- The user wants stronger academic tone, cleaner flow, more natural phrasing, or more specific reviewer-facing language
- The user is editing a local text block rather than running a full paper gate flow

## Do not use

- The main question is whether the science stands up → use `$paper-logic`
- The task is whole-paper triage, submission-readiness review, or dimension review → use `$paper-reviewer`
- The task is manuscript execution from reviewer comments, gate files, or strategic `delete / narrow / move_to_appendix / de_emphasize` decisions → use `$paper-reviser`
- The task is mainly figure/table presentation → use `$paper-visuals`
- The task is primarily generic prose naturalization outside manuscript context → use `$humanizer`
- The task is rebuttal/response-letter **orchestration** (coordinating manuscript edits + writing the letter) → use `$paper-reviser`

## Routing defaults

Choose this skill only when the paper-facing task is mainly local text work:

- rewrite or polish a named text block
- improve tone, flow, wording, or naturalness
- keep the current claim and evidence boundaries fixed

Do not choose this skill when the user is really asking:

- "能不能投" or "帮我审一下" -> `$paper-reviewer`
- "根据 reviewer comments 修改" or "该删就删 / 藏到附录" -> `$paper-reviser`

## Handoff to humanizer

Route to `$humanizer` when one or more of these become true:

- the text is not actually paper content, but a generic paragraph, email, post, statement, docs prose, or application-style writing
- the user mainly wants “去模板腔 / 更自然 / 别像 AI 写的”, without manuscript-specific constraints
- there is no need to preserve cross-section paper logic, notation consistency, or reviewer-facing academic framing
- the task is about local prose naturalization rather than paper-genre revision

When handing off:

- state briefly that the request is better treated as generic prose naturalization
- keep any paper-adjacent constraints the user gave, such as formal tone or restrained wording
- let `$humanizer` own the style-naturalization pass

## Required workflow

1. Identify the text scope and fixed scientific constraints.
2. Preserve technical meaning unless the user asks to change it.
3. Apply venue-aware section defaults unless the user or target journal requires otherwise:
   - abstract: one paragraph by default; avoid mathematical notation unless indispensable
   - introduction: end with a roadmap paragraph that previews the remaining sections when the paper genre expects it
   - conclusion: default to two paragraphs or fewer unless a venue requires a longer discussion-style ending
4. Improve:
   - clarity
   - flow
   - precision
   - terminology consistency
   - reviewer-facing professionalism
5. If captions are involved, keep them consistent with `$paper-visuals`.
6. In gate-chain mode, consume only the markdown packet plus manuscript text
   needed for the current gate instead of carrying forward loose prior-thread
   narration.

## Output defaults

Use `文字修改建议` or `文字修订记录`:
- revised text
- revision record (修改意见): use the table format from `$humanizer`
- remaining risks

When the request is SCI / top-journal / 顶刊级 polishing, append a compact **section checklist status** after each rewritten section:
- `Abstract`: one paragraph / notation reduced / key result present / implication present
- `Introduction`: gap clear / contribution clear / final roadmap paragraph present when appropriate
- `Conclusion`: ≤2 paragraphs / claim-evidence match / limitations or scope stated
- `Global`: terminology consistency / claim strength calibrated / reviewer-facing risks

## Rebuttal & Response Letter Workflow

> **Ownership note**: Rebuttal and response letter **orchestration** (coordinating manuscript edits alongside the letter) is owned by `$paper-reviser`. This skill handles rebuttal **prose polish** when delegated by `$paper-reviser`, or when the user explicitly asks only for writing-quality help on a response letter without needing manuscript edits.

When handling rebuttal prose polish:

1. **Structure by reviewer**: group responses by Reviewer #1, #2, etc.
2. **For each comment**:
   - Quote the original reviewer comment
   - Draft a response that is: respectful, specific, evidence-linked, non-defensive
   - Reference exact page/line/figure numbers for changes made
   - If declining, give clear technical rationale without being dismissive
3. **Tone principles**:
   - Never argue; explain with evidence
   - Acknowledge valid concerns explicitly before responding
   - Use language like "We have revised..." / "We appreciate this suggestion and have..."
   - Avoid "We disagree" — prefer "We would like to clarify..."
4. **Formatting**: use diff-style or color markup to highlight changes in the manuscript
5. If the task involves coordinating actual manuscript edits (not just writing the letter), route to `$paper-reviser`

## Citation Hallucination Prevention

> [!CAUTION]
> AI-generated citations have **~40% error rate**. Never write BibTeX from memory.

| Rule | Detail |
|------|--------|
| **Never fabricate** | Do not invent titles, authors, venues, or DOIs |
| **Verify every citation** | Use Semantic Scholar, Google Scholar, or CrossRef to confirm existence |
| **Use placeholder if unverifiable** | `[CITATION NEEDED: claim about X]` — let the human fill it |
| **Prefer DOI links** | Machine-verifiable; link rot-resistant |
| **Mark uncertain citations** | If you recall a paper but can't verify, say so explicitly |

Route citation retrieval to `$academic-search` or `$citation-management` when systematic verification is needed.

## Reviewer Reading Behavior & Time Allocation

Understanding how reviewers actually read papers helps prioritize effort:

| Paper Section | % Reviewers Who Read | Implication |
|---|---|---|
| Abstract | 100% | Must be perfect |
| Introduction | 90%+ (skimmed) | Front-load contribution |
| Figures | Examined before methods | Figure 1 is critical |
| Methods | Only if interested | Don't bury the lede |
| Appendix | Rarely | Put only supplementary details |

**Time allocation** (from Neel Nanda): spend approximately **equal time** on each of:
1. The abstract
2. The introduction
3. The figures
4. Everything else combined

## Hard constraints

- Do not quietly alter scientific claims.
- Do not over-polish into vague generic language.
- Do not fabricate or invent citations, DOIs, or paper references.
- **Anti-AIGC Enforcement**: Strictly avoid meta-discourse ("This section presents", "Next, we discuss") and transition clusters ("Notably", "Moreover", "Furthermore") that signal automated filler.
- **Structural Integrity**: Never introduce or leave duplicate `\section` or `\paragraph` headers during surgical edits.

## Section-level writing strategy

Each paper section has distinct goals, structures, and common pitfalls.
See [references/section-by-section.md](references/section-by-section.md) for detailed guidance on
section structure, sentence-level clarity (Gopen & Swan 7 principles), and micro-level tips:
- Abstract (elevator pitch, quantitative result requirement)
- Introduction (4–5 paragraph contribution pattern + final roadmap paragraph)
- Related Work (cluster by method family, positioning)
- Method (notation discipline, assumption framing)
- Experiments (Mean±Std, ablation coverage, computational cost)
- Discussion (observation vs interpretation)
- Conclusion (design-principle-level insights, usually ≤2 paragraphs)

## De-AIGC & Rhythm Guidelines

This skill uses the [shared de-AIGC standards](../writing-skills/resources/de-aigc-standards.md) and `$humanizer` as its stylistic engine. 

For academic papers, apply these standards while maintaining:
1. **Scientific Integrity**: Never sacrifice technical clarity for "natural" rhythm.
2. **Reviewer-Facing Tone**: Keep the persona of a senior, cautious researcher.
3. **Notation Discipline**: Avoid introducing colloquialisms into variable/method descriptions.

### Core Workflow:
1. Revise for **Logic and Evidence** first.
2. Apply the **shared de-AIGC standards** to break the "AI drone".
3. Provide the **Revision Record** in the delivery.

### Top-journal default constraints

Use these defaults when the user asks for SCI / top-journal / 顶刊级 polishing and does not provide stricter venue rules:

- **Abstract**: keep it to a single paragraph; make it self-contained; prefer plain scientific language over symbolic notation; if a symbol is unavoidable, define it implicitly with surrounding words.
- **Introduction**: make the final paragraph a roadmap of the paper structure, e.g. "The remainder of this paper is organized as follows...", especially for engineering and methods papers.
- **Conclusion**: keep it to one or two paragraphs; first paragraph = main findings and significance under evidence constraints; optional second paragraph = limitations, scope, and concrete future work.
- **Claim strength**: reduce rhetorical force before deleting technical nuance; top-journal polish should sound more auditable, not more promotional.

### Executable top-journal rewrite checklist

Run this checklist before finalizing any SCI / top-journal / 顶刊级 rewrite.

#### Abstract

- [ ] Single paragraph unless venue requires structured headings
- [ ] Opens with field context or problem, not self-referential filler
- [ ] States the gap, method/action, and core result
- [ ] Includes at least one concrete result signal when the source text supports it
- [ ] Avoids mathematical notation unless indispensable
- [ ] Final sentence states implication without overstating scope

#### Introduction

- [ ] Broad context is short and directly relevant
- [ ] Specific gap is explicit and auditable
- [ ] Contribution statement answers that gap directly
- [ ] Claim strength matches available evidence
- [ ] Final paragraph provides paper organization when the genre expects a roadmap
- [ ] Roadmap paragraph describes section roles rather than repeating contributions

#### Conclusion

- [ ] Total length is one or two paragraphs unless venue rules say otherwise
- [ ] First paragraph answers the research question and contribution boundary
- [ ] No new results or inflated novelty claims appear in the closing
- [ ] Limitations, scope conditions, or failure boundaries are acknowledged
- [ ] Future work, if present, is concrete rather than ceremonial

#### Global pass

- [ ] Terminology is consistent across sections
- [ ] Quantifiers, hedging, and claim verbs match evidence strength
- [ ] Cross-subfield readability is improved where possible
- [ ] Promotional phrasing is removed before technical nuance is cut
- [ ] **AIGC Signal Check**: Meta-discourse removed; transition-adverb density reduced.
- [ ] **Rhythm Check**: Paragraphs contain a mix of sentence lengths and structures.
- [ ] Remaining reviewer-facing risks are explicitly surfaced if not fully fixable

## Claim strength ladder

Match language to evidence strength:

| Evidence | Language |
|---|---|
| Strong (multiple experiments, significance) | "demonstrate", "show", "establish" |
| Moderate (consistent trends) | "suggest", "indicate", "provide evidence" |
| Weak (preliminary) | "appear to", "may", "initial results hint" |
| Speculation | "we hypothesize", "it is plausible" |

## Scientific Sincerity & Senior Persona

Top-journal reviewers look for "Scientific Sincerity" — the feeling that the author is an expert who lived through the data, not a machine summarizing a textbook.

### 1. The Persona: The Cautious Expert
- **Tone**: Professional, restrained, but firm on evidence.
- **Action**: Use "We observed" or "Our results suggest" over "This proves".
- **Hedging**: Hedge on generalizability ("under these specific conditions"), not on the data itself.

### 2. Narrative of Friction
- Describe the difficulties in the experiment/method.
- "The initial sensor calibration drifted significantly; we addressed this by..."
- This "friction" is a 99% human signal that AI rarely generates without specific prompting.

### 3. Asymmetric Detail
- AI gives equal weight to every step. 
- Humans deep-dive into the "hard parts" and breeze through the "standard parts".
- Ensure the methodology reflects where the *actual* effort was spent.

- 中文论文默认规范：GB/T 7714 引用格式、中文标点、"本文提出" 而非 "我们提出"
- 英文论文中出现中文术语时，首次使用需附英文翻译或解释
- 中文学术语气通常更克制，避免 "首次""创新性" 等强硬表述除非有充分证据
- 英文 hedging language 比中文更丰富，参考 claim strength ladder

## Rebuttal & response letter reference

See [references/rebuttal-patterns.md](references/rebuttal-patterns.md) for:
- 6 common reviewer attack patterns and response templates
- Response letter structure template
- Tone principles and phrase recommendations
- "强制进行论文写作深度审计 / 检查逻辑严密性与引用准确性。"
- "Use $execution-audit-codex to audit this abstract for argument-fidelity idealism."
