---
name: paper-logic
description: |
  Check whether a paper's claims are actually supported. Use for requests like
  "看论文逻辑", "创新性够不够", "claims 和 evidence 对不对齐", "实验站不站得住", or
  "审稿人会怎么从科学逻辑上打". This skill audits novelty positioning, claim
  ceiling, evidence coverage, ablation isolation, and experiment framing. It is a
  specialist lane for scientific defensibility, not a whole-paper product by itself.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 看论文逻辑
  - 创新性够不够
  - claims 和 evidence 对不对齐
  - 实验站不站得住
  - 审稿人会怎么攻击逻辑
  - 看 claim ceiling
  - 看实验支撑够不够
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags: [paper, logic, novelty, evidence, experiments, reviewer]
framework_roles:
  - detector
  - executor
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: medium
source: local
---

# Paper Logic

This skill owns one narrow question: does the science hold up under review?

In a full paper workflow, this skill is usually a specialist sidecar lane under
`$paper-workbench`, `$paper-reviewer`, or `$paper-reviser`, not the main
orchestration lane.

## Use this when

- The main question is claim vs evidence alignment
- The user wants novelty, contribution level, or experiment support judged
- The user wants to know whether a point is fixable by rewriting or blocked by missing evidence
- `$paper-reviewer` or `$paper-reviser` needs a deep logic sub-pass

## Do not use

- The user wants one front door for the manuscript task -> use `$paper-workbench`
- The user wants the whole paper judged as a submission package -> use `$paper-reviewer`
- The user wants direct manuscript edits across multiple surfaces -> use `$paper-reviser`
- The user wants local prose polish -> use `$paper-writing`
- The user wants only citation hygiene -> use `$citation-management`

## What this skill should deliver

Keep the output practical:

1. which claim or argument is weak
2. what evidence is missing or mismatched
3. whether the shortest honest fix is rewrite, reframe, or new experiment

## Review focus

Prioritize these checks:

- novelty positioning
- claim ceiling
- experiment coverage
- ablation isolation
- statistical or comparison fairness
- theory or derivation only when the surviving claim truly depends on it

## Hard rules

- Do not fabricate evidence
- Do not solve evidence gaps with wording tricks
- Do not soften scientific flaws into generic "could improve" language
- If the issue is really whole-paper readiness, hand back to `$paper-reviewer`
