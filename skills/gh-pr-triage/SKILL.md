---
name: gh-pr-triage
description: |
  Triage GitHub pull requests by collecting PR metadata, comments, reviewer state, changed files,
  and next-action summaries. Use when the user wants a quick PR 状态梳理, PR review summary,
  reviewer feedback digest, changed-file digest, or PR-level follow-up plan rather than full
  comment-resolution or CI debugging.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - quick PR 状态梳理
  - PR review summary
  - pull request summary
  - reviewer feedback digest
  - changed-file digest
  - CI debugging
  - github PR
  - PR triage
  - pull request
  - changed files summary
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - github
    - pr
    - triage
    - reviewer
    - changed-files
risk: low
source: local
---

# gh-pr-triage

This skill owns lightweight GitHub PR triage and summary work.

## When to use

- The user wants a quick summary of a PR state, comments, or changed-file surface
- The task is to understand what a PR needs next before deeper execution

## Do not use

- Review-thread resolution work -> use `$gh-address-comments`
- Broken GitHub Actions analysis -> use `$gh-fix-ci`
- Repository history or timeline research -> use `$github-investigator`
- Generic framework / routing / skill-boundary review -> use `$skill-framework-developer` with `$code-review`
