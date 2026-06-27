---

allowed_tools:
- shell
- git
- rust
approval_required_tools:
- git push
description: Address GitHub PR review comments and lightweight PR triage summaries with gh-source-gate.
metadata:
  platforms:
  - supported
  short-description: Address comments in a GitHub PR review
  tags:
  - github
  - pull-request
  - review-comments
  - gh-cli
  - code-review
  version: '2.0.0'
name: gh-address-comments
scene: code_review
network_access: conditional
risk: medium
routing_gate: source
routing_layer: L0
routing_owner: gate
routing_priority: P2
runtime_requirements:
  commands:
  - cargo
  - gh
  - git
session_start: required
source: project
trigger_hints:
- /gh-address-comments
- PR comments
- PR review summary
- PR triage
- PR 评论回复
- address PR feedback
- address comments
- changed files summary
- changed-file digest
- pull request summary
- review comments
- review feedback
- reviewer feedback digest
- reviewer 意见处理
---
# gh-address-comments

At conversation start or first turn, check this source gate before ordinary domain owners when the request is driven by external evidence such as Sentry data, PR comments, or failing checks.


This skill owns the workflow for turning GitHub PR feedback and lightweight PR
triage evidence into an actionable, numbered fix list and then applying the
selected fixes cleanly.

Default helper:

```bash
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/gh_source_gate_rs/Cargo.toml --bin gh-source-gate -- \
  fetch-comments --repo . --json
```

Use `--open-only` when the next step should focus only on unresolved,
non-outdated review threads.

## Priority routing rule

If the task is triggered by GitHub PR review comments, review threads, PR
conversation comments, reviewer digests, changed-file digests, or PR-level
summary requests on the current branch, check this skill before generic git
workflow or implementation skills.

In that case:

1. this skill owns fetching and structuring the actual PR feedback source
2. fix work can follow only after the comment queue is clear

## When to use

- The user wants to fetch or summarize comments, reviewer state, or PR metadata on the open PR for the current branch
- The user wants a lightweight PR summary, reviewer feedback digest, changed-file digest, or next-action triage without CI debugging
- The user wants to address review comments or unresolved review threads
- The user asks which GitHub comments should be fixed first
- The user wants a clean follow-up after code review feedback
- Best for requests like:
  - "拉一下这个 PR 的 review comments，帮我整理一下"
  - "处理 GitHub 上的 review comments"
  - "把这个 PR 里的 comments 编号总结后再修"

## Do not use

- The task is mainly about failing CI checks rather than human review feedback → use `$gh-fix-ci`
- The user wants generic git branching, rebasing, or publishing help → use `/gitx`
- There is no relevant PR context and the task is not review-comment driven
- The user specifically wants GitHub review automation outside the current branch PR workflow

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).

## Exit Criteria

- 所有 reviewer comments 已逐条回复（resolved 或 replied）
- PR 状态已更新（comment 已 post 或 commit 已 push）
- 用户确认回复策略（直接修改 / 解释说明 / 标记 wontfix）

## Hard constraints

- 必须使用 `gh-source-gate` CLI 获取评论源数据，不得凭记忆判断评论内容
- 评论优先级排序必须基于严重度（blocker > nit），不得按时间顺序处理
- 每条评论的处理结果必须显式标注（fixed / explained / wontfix），不得遗漏
- git push 前必须经过用户确认（`approval_required_tools: git push`）
- 超出当前 PR 范围的评论（如跨仓库建议），必须标注为 out-of-scope
