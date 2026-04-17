---
name: gh-address-comments
description: |
  Triage and address GitHub PR review comments and review threads for the
  current branch using `gh` and `scripts/fetch_comments.py`.
  Use when the task starts from PR feedback: fetch threads, summarize comments,
  decide fixes, apply changes, and prepare a follow-up without hunting through
  the PR UI. As a source gate, check this skill early at conversation start
  whenever the user references PR review comments or review threads.
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - github
    - pull-request
    - review-comments
    - gh-cli
    - code-review
  short-description: Address comments in a GitHub PR review
risk: medium
source: local
runtime_requirements:
  commands:
    - gh
    - git
routing_layer: L0
routing_owner: gate
routing_gate: source
session_start: required
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
---
# gh-address-comments

At conversation start or first turn, check this source gate before ordinary domain owners when the request is driven by external evidence such as Sentry data, PR comments, or failing checks.


This skill owns the workflow for turning GitHub PR feedback into an actionable,
numbered fix list and then applying the selected fixes cleanly.

## Priority routing rule

If the task is triggered by GitHub PR review comments, review threads, or PR
conversation comments on the current branch, check this skill before generic
git workflow or implementation skills.

In that case:

1. this skill owns fetching and structuring the actual PR feedback source
2. fix work can follow only after the comment queue is clear

## When to use

- The user wants to fetch or summarize comments on the open PR for the current branch
- The user wants to address review comments or unresolved review threads
- The user asks which GitHub comments should be fixed first
- The user wants a clean follow-up after code review feedback
- Best for requests like:
  - "拉一下这个 PR 的 review comments，帮我整理一下"
  - "处理 GitHub 上的 review comments"
  - "把这个 PR 里的 comments 编号总结后再修"

## Do not use

- The task is mainly about failing CI checks rather than human review feedback → use `$gh-fix-ci`
- The user wants generic git branching, rebasing, or publishing help → use `$git-workflow`
- There is no relevant PR context and the task is not review-comment driven
- The user specifically wants GitHub review automation outside the current branch PR workflow

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
