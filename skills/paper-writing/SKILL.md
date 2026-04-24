---
name: paper-writing
description: |
  Write, restructure, or polish a bounded academic-paper text unit after the
  claim/evidence boundary is known. Use for "论文写作", "paper_writing",
  "论文润色", "英文论文润色", "SCI润色", "学术润色", "manuscript editing",
  "academic writing", "scientific writing", "帮我写摘要", "根据要点写 abstract",
  "写 introduction", "重写 introduction", "改 related work 文字",
  "科研讲故事", "论文故事线", "根据ref学习写法", "改段落逻辑", "改 caption", "cover letter", "回复信润色",
  "response to reviewers", "只改表达不改 claim", or "不是整篇 review，只做文字/结构精修".
  This skill owns reader-facing prose: section purpose, paragraph flow,
  research storytelling, claim calibration, academic tone, and sentence clarity. It must not invent
  science, citations, results, or reviewer-facing promises.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 论文写作
  - paper_writing
  - 写论文
  - 论文润色
  - 论文语言润色
  - 英文论文润色
  - 英文润色
  - 论文修改
  - 论文精修
  - 论文改写
  - 论文表达优化
  - SCI润色
  - SCI论文润色
  - SCI论文写作
  - SCI语言润色
  - journal manuscript polish
  - journal paper editing
  - 学术润色
  - 学术写作
  - 学术表达润色
  - 学术英语润色
  - 写摘要
  - 帮我写 abstract
  - 根据要点写摘要
  - 摘要重写
  - abstract writing
  - abstract rewrite
  - abstract polish
  - 写 introduction
  - 重写 introduction
  - 写引言
  - 重写引言
  - introduction rewrite
  - introduction editing
  - 润色摘要
  - 润色引言
  - 改摘要表达
  - 改引言表达
  - 只润色摘要和引言表达
  - 只润色摘要和引言
  - 只改表达不改 claim
  - 别改 claim
  - 不做整篇 review
  - 只做文字润色
  - 只润色这一段
  - 改 related work 文字
  - 改 conclusion 表述
  - 改 caption
  - 改 figure caption
  - 改 table caption
  - 图注润色
  - 表注润色
  - 回复信润色
  - 审稿回复润色
  - 回复审稿人
  - rebuttal 润色
  - rebuttal letter
  - response letter
  - response to reviewers
  - reviewer response
  - author response
  - point-by-point response
  - 投稿 cover letter
  - submission cover letter
  - cover letter polish
  - 文字精修
  - 改段落逻辑
  - 科研讲故事
  - 论文故事线
  - 论文叙事
  - 研究故事线
  - 根据ref学习写法
  - 学习目标期刊写法
  - 按目标期刊风格改写
  - ref学习
  - story line
  - research storytelling
  - paper narrative
  - scientific storytelling
  - target journal style
  - 段落重组
  - 段落衔接
  - 逻辑衔接
  - 论文结构润色
  - 学术表达
  - 论文降AI味
  - 去AI味
  - 降低AI味
  - AIGC润色
  - AI味修改
  - manuscript polish
  - manuscript editing
  - paper editing
  - paper polishing
  - academic writing
  - scientific writing
  - research paper writing
  - academic paper polish
  - academic polishing
  - scientific manuscript
  - proofread paper
  - proofread manuscript
  - copyedit manuscript
  - language editing
  - improve clarity
  - improve flow
  - improve coherence
  - claim calibration
  - hedge claims
  - revise manuscript language
  - polish manuscript
  - rewrite abstract
  - polish introduction
  - draft from notes
  - draft manuscript section
  - methods writing
  - methodology writing
  - results writing
  - discussion writing
  - conclusion rewrite
metadata:
  version: "2.6.0"
  platforms: [codex]
  tags: [paper, writing, rewrite, abstract, introduction, conclusion, caption, rebuttal]
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

# Paper Writing

This skill owns bounded manuscript prose. It can polish existing text, rewrite a
section, or draft from supplied notes, but only inside facts the user has given,
facts already present in the manuscript, or verified sources explicitly provided
for the task.

## Use this when

- The user wants one section, one paragraph, or one text block rewritten
- The user provides enough factual ingredients to draft a local section from notes
- The main job is reader-facing structure, flow, academic tone, or tighter wording
- The user wants a stronger research story: gap, tension, contribution, evidence sequence, and takeaway
- The user has a target-journal ref-learning brief or comparable-paper set to emulate structurally
- The claim, evidence, and scope are fixed or can be extracted from the provided text
- The user wants response-letter or rebuttal prose polish only, without coordinating manuscript decisions
- The user wants a submission cover letter or other bounded journal-facing prose
- The user wants paper-specific "de-AI" cleanup while preserving scientific claims

## Do not use

- The user wants one front door for a manuscript task -> use `$paper-workbench`
- The user wants to know whether the paper stands up scientifically -> use `$paper-logic`
- The user wants submission-facing judgment -> use `$paper-reviewer`
- The user wants reviewer-comment execution, claim narrowing, or appendix routing -> use `$paper-reviser`
- The task needs new literature, missing citations, or novelty search -> use `$literature-synthesis`
- The task first needs 20 target-journal references downloaded and learned -> use `$literature-synthesis`
- The task is mainly figure or table presentation -> use `$paper-visuals`
- The task is generic de-AI naturalization outside paper context -> use `$humanizer`
- The task is citation verification or reference-list repair -> use `$citation-management`

## Operating Model

1. Lock the boundary: target section, audience/venue if known, allowed facts, core claim, and forbidden additions.
2. If a ref-learning brief exists, extract the venue story norm before drafting: opening move, gap type, contribution posture, evidence order, and limitation style.
3. Diagnose the reader problem before rewriting: unclear gap, weak tension, zig-zag flow, weak topic sentence, overclaim, missing transition, dense sentence, or inconsistent term.
4. Rewrite from large to small: paper story -> section story -> paragraph logic -> sentence clarity -> word choice.
5. Calibrate every strong verb to the actual evidence.
6. If the target venue is known, adapt abstract format, section order, word budget, and AI-use disclosure to that venue.
7. Choose the smallest useful output: revised text only for simple polish; revised text plus notes only when risks remain.
8. Deliver revised text first. Add at most three short notes only for unresolved risk, placeholders, or claim/evidence mismatch.

If invoked inside the protocol-backed paper workflow, follow the active paper
state from [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md), but do not
surface internal gate mechanics unless the user asks.

## Modes

- **Polish**: preserve claims and section order; improve clarity, flow, tone, grammar, and "paper voice".
- **Structural rewrite**: move/split/merge sentences or paragraphs when the same facts need a better reader path.
- **Draft from notes**: write prose only from supplied facts; mark missing numbers, citations, or definitions with `[VERIFY: ...]`.
- **Storyline rewrite**: rebuild the problem-gap-move-evidence-takeaway path while preserving the scientific boundary.
- **Ref-guided venue imitation**: use a target-journal corpus to match story architecture, evidence order, and claim tone without copying sentences.
- **Story card alignment**: keep abstract, introduction, results, discussion, and conclusion consistent with the same one-sentence spine.
- **Response prose**: turn reviewer-facing decisions into concise, respectful, evidence-backed text.
- **Submission cover letter**: summarize the manuscript's fit, contribution, and compliance points from supplied facts only.
- **Paper de-AI cleanup**: remove formulaic AI texture by adding specificity, real transitions, concrete agents/actions, and claim-safe nuance; do not disguise fabricated content.

## References

- For section-specific patterns, read [`references/section-by-section.md`](references/section-by-section.md).
- For research storytelling and ref-guided writing, read [`references/storytelling-patterns.md`](references/storytelling-patterns.md).
- For response letters and rebuttals, read [`references/rebuttal-patterns.md`](references/rebuttal-patterns.md).
- For non-trivial rewrites, drafting from notes, de-AI cleanup, or submission-facing work, read [`references/revision-playbook.md`](references/revision-playbook.md).

## Rewrite Priorities

Improve, in this order:

1. clarity
2. precision
3. flow
4. tone
5. terminology consistency

## Core Heuristics

- One paper should have one central contribution that readers can repeat in plain words.
- A strong paper story is not decoration; it is the shortest path from known field pain to the exact evidence this paper can defend.
- Each paragraph should do one job and answer "why this now?" at the start and "so what?" at the end.
- Put familiar context before new information; put the sentence's important point near the end.
- Keep subject and verb close; turn nominalizations into verbs when possible.
- Results report observations; discussion interprets them. Do not sneak new results into discussion or conclusion.
- Methods must be clear enough for trust and, where appropriate, replication.
- If AI-assisted writing disclosure is relevant for the venue, remind the user to follow the venue's policy.

## Quality Gate

Before final delivery, silently check:

- **Claim safety**: no new result, citation, dataset, baseline, or promise was invented.
- **Reader path**: the revised text makes the problem, gap, move, evidence, and takeaway easier to follow.
- **Story tension**: the text makes clear why the current gap matters now and why the paper's move is the right next step.
- **Cross-section alignment**: abstract, introduction, contribution bullets, results, and conclusion tell the same bounded story.
- **Evidence fit**: strong verbs match the strength of the supplied evidence.
- **Section job**: the text does what this section is supposed to do, not a neighboring section's job.
- **Venue risk**: obvious word-count, disclosure, confidentiality, or figure/image-policy risks are flagged.

## Hard Rules

- Do not quietly change scientific claims
- Do not invent citations, results, or evidence
- Do not turn weak science into confident prose
- Do not hide missing information; use `[VERIFY: ...]` instead
- Do not paraphrase copied text to hide plagiarism or bypass similarity checks
- Do not upload or reuse confidential third-party manuscript text outside the current authorized workspace
- Do not assume AI-writing disclosure rules; check the target venue when submission is in scope
- If the real problem is scope or evidence, hand the task back to `$paper-reviser` or `$paper-logic`
