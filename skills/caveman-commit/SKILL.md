---
name: caveman-commit
description: >
  Caveman 风格 git commit 消息。Conventional Commits 格式，≤50 字 subject。
  Trigger: /caveman-commit
scene: git
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P4
session_start: on_demand
trigger_hints:
- /caveman-commit
- caveman commit
- 压缩提交
- 简单提交
- 简短 commit
---

# Caveman Commit

Caveman-style git commit messages. Conventional Commits format.

## Rules

- Subject: ≤50 chars, Conventional Commits type + brief description
- Body: caveman terse — drop filler, keep substance
- No period in subject line
- No tool-call narration

Pattern: `type(scope): short description`

Not: "feat(auth): This commit adds a new authentication middleware that validates JWT tokens..."
Yes: "feat(auth): add JWT validation middleware"

## Trigger

`/caveman-commit` or when user says "caveman commit please" / "简短提交消息"
