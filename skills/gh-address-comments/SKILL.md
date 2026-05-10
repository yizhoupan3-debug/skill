---
name: gh-address-comments
description: Address GitHub PR review comments and lightweight PR triage summaries with gh-source-gate.
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
    - cargo
    - gh
    - git
routing_layer: L0
routing_owner: gate
routing_gate: source
session_start: required
trigger_hints:
  - github
  - pull request
  - pull request summary
  - PR review summary
  - PR triage
  - reviewer feedback digest
  - changed-file digest
  - changed files summary
  - review comments
  - gh cli
  - code review
allowed_tools:
  - shell
  - git
  - rust
approval_required_tools:
  - git push
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - comment_digest.md
  - EVIDENCE_INDEX.json
  - TRACE_METADATA.json

---

# gh-address-comments

At conversation start or first turn, check this source gate before ordinary domain owners when the request is driven by external evidence such as Sentry data, PR comments, or failing checks.


This skill owns the workflow for turning GitHub PR feedback and lightweight PR
triage evidence into an actionable, numbered fix list and then applying the
selected fixes cleanly.

Default helper:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/gh_source_gate_rs/Cargo.toml --bin gh-source-gate -- \
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
