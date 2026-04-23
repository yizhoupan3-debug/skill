---
name: paper-writing
description: |
  Polish local paper text after the claim and evidence boundary is already fixed.
  Use for requests like "润色摘要", "改引言表达", "重写 conclusion", "改 related work
  文字", "改 caption", "只润色摘要和引言", "只改表达不改 claim", or "不是整篇
  review，只做文字润色". This skill improves tone, flow, precision, and
  readability for a named text block without changing the science.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
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
  - 回复信润色
  - 文字精修
  - rewrite abstract
  - polish introduction
metadata:
  version: "2.2.0"
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

This skill owns local manuscript rewriting after the scientific boundary is
already decided.

In the protocol-backed workflow, this skill is usually a bounded sidecar lane
for local text blocks after `$paper-workbench` or `$paper-reviser` has already
frozen the claim and evidence boundary.

## Use this when

- The user wants one section, one paragraph, or one text block rewritten
- The main job is clearer academic tone, smoother flow, tighter wording, or less awkward phrasing
- The claim, evidence, and scope are already fixed
- The user wants response-letter or rebuttal prose polish only, without coordinating manuscript decisions

## Do not use

- The user wants one front door for a manuscript task -> use `$paper-workbench`
- The user wants to know whether the paper stands up scientifically -> use `$paper-logic`
- The user wants submission-facing judgment -> use `$paper-reviewer`
- The user wants reviewer-comment execution, claim narrowing, or appendix routing -> use `$paper-reviser`
- The task is mainly figure or table presentation -> use `$paper-visuals`
- The task is generic de-AI naturalization outside paper context -> use `$humanizer`

## User-facing contract

Think of this skill as "只润这一段，不动 claim".

Default delivery should stay simple:

1. revised text
2. a short note on remaining risk if the source claim is still too strong or too vague

If invoked inside the protocol-backed paper workflow, follow the active paper
state from [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md), but do not
surface internal gate mechanics unless the user asks.

## Rewrite priorities

Improve, in this order:

1. clarity
2. precision
3. flow
4. tone
5. terminology consistency

## Hard rules

- Do not quietly change scientific claims
- Do not invent citations, results, or evidence
- Do not turn weak science into confident prose
- If the real problem is scope or evidence, hand the task back to `$paper-reviser` or `$paper-logic`
