---
name: gh-fix-ci
description: Triage and fix failing GitHub Actions PR checks with gh-source-gate.
metadata:
  version: "2.0.0"
  platforms: [supported]
  tags:
    - github
    - ci
    - github-actions
    - gh-cli
    - pull-request

runtime_requirements:
  commands:
    - cargo
    - gh
    - git
routing_layer: L0
routing_owner: gate
routing_gate: source
routing_priority: P2
session_start: required
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - github
  - ci
  - github actions
  - gh cli
  - pull request
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
  - ci_failure_digest.md
  - EVIDENCE_INDEX.json
  - TRACE_METADATA.json

---

# gh-fix-ci

At conversation start or first turn, check this source gate before ordinary domain owners when the request is driven by external evidence such as Sentry data, PR comments, or failing checks.


This skill owns GitHub Actions PR-check triage: turning failing checks into a ranked failure summary and a fix plan.

Default helper:

```bash
cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/gh_source_gate_rs/Cargo.toml --bin gh-source-gate -- \
  inspect-pr-checks --repo . --json
```

在仓库根执行；若在子目录，请把 `--manifest-path` 写成指向仓库根的相对或绝对路径。

## Priority routing rule

If the request is about a PR's failing GitHub Actions checks, broken PR status,
or CI logs on the current branch PR, check this skill before generic debugging,
git workflow, or test advice.

In that case:

1. this skill owns source-grounded CI evidence collection and failure
   classification
2. implementation or deeper debugging should follow the triage result

## When to use

- The user wants to inspect failing GitHub PR checks
- The user wants logs summarized and likely causes identified
- The user wants to debug GitHub Actions failures on the current branch PR
- The user wants a fix plan before code changes
- Best for requests like:
  - "看下这个 PR 为什么 CI 挂了"
  - "帮我分析 GitHub Actions 失败日志"
  - "先总结失败原因，再决定要不要修"

## Do not use

- The task is about human review comments rather than CI → use `$gh-address-comments`
- The failing provider is external and not GitHub Actions; report the URL only
- The task is generic git/release workflow rather than CI triage
- The user explicitly wants immediate implementation without approval after triage

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).

## Exit Criteria

- CI 状态从红变绿（或已确认为 flaky/infra issue）
- 失败原因已根因分析并记录
- fix 已 push 并通过 CI 验证

## Hard constraints

- 必须使用 `gh-source-gate` CLI 获取 CI 证据，不得凭记忆或截图判断失败原因
- flaky test 不得自动豁免——必须标记为 flaky 并给出证据（多次重跑结果）
- fix 必须经过本地验证后再 push，不得直接 push 未验证的修复
- CI 日志过长时（>500 行），必须先摘要再分析，不得将原始日志全部呈现
- 超出当前仓库范围的 CI 问题（如 GitHub Actions 平台故障），必须明确标注为外部依赖并建议等待
