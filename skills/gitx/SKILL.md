---
name: gitx
description: |
  Codex 里的 Git 一条龙收口入口。Use when the user says `$gitx` / `/gitx` / `gitx`,
  or explicitly wants review、修复、整理、提交、合并 worktree、推送一次做完。
  This skill treats that request as an end-to-end Git closeout workflow instead of a
  single `git status` reply.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Run the safe Git review-fix-tidy-commit-merge-push workflow end to end.
trigger_hints:
  - $gitx
  - /gitx
  - gitx
  - git 一条龙
  - review 修复 整理 提交 推送
  - 合并 worktree 并推送
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - git
    - git-closeout
    - review
    - commit
    - worktree
    - push
risk: medium
source: local
filesystem_scope:
  - repo
network_access: conditional
bridge_behavior: mobile_complete_once
---

# gitx

`gitx` 是给 Codex 用的 Git 收口快捷入口。

## When to use

- 用户明确说 `$gitx`、`/gitx`、`gitx`
- 用户要把 review、修复、整理、提交、合并 worktree、推送当成一次连续动作做完
- 当前重点是把仓库安全收口，而不是只回答某个 Git 概念

## Do not use

- 只是问 Git 概念、命令语法、分支策略建议 -> [`$git-workflow`](../git-workflow/SKILL.md)
- 根因还不清楚，先要查为什么坏了 -> `$systematic-debugging`
- 只是做 PR 评论收口 -> `$gh-address-comments`
- 只是做纯代码 review，不涉及提交流程 -> `$code-review`

## Default contract

把 `$gitx` 视为用户对当前仓库发出的“安全一条龙收口”授权，默认目标是：

1. 先看清真实 Git 状态，而不是直接提交
2. 先 review 再 fix
3. 先整理脏改动和 worktree，再决定怎么提交
4. 最后把应该推送的分支安全推上去

如果当前目录不是 Git 仓库，不要擅自初始化；直接说明不是仓库并停下。

## Execution tiers

- 只诊断：`python3 scripts/git_safety.py doctor`
- 出收口步骤：`python3 scripts/git_safety.py publish-plan`
- 自动执行到 merge：`python3 scripts/git_safety.py auto-closeout`
- 自动执行到 push：`python3 scripts/git_safety.py auto-closeout --push`

## Required workflow

1. 先跑 gitx 诊断，而不是上来就提交：
   - `python3 scripts/git_safety.py doctor`
   - `python3 scripts/git_safety.py publish-plan`
2. 需要看原始面时，再补：
   - `python3 scripts/git_safety.py status`
   - `git status --short --branch`
   - `git worktree list --porcelain`
   - `git stash list`
3. 若发现脏改动直接堆在 `main`、存在 stash、或 worktree 头部不一致：
   - 先做 checkpoint，必要时用 `python3 scripts/git_safety.py start-topic <branch>`
   - 不要直接在混乱状态下提交
4. 对待提交改动做 review：
   - 先找明显 bug、回归、脏文件、生成物噪音、遗漏测试
   - 需要时先修复再继续
5. 低风险场景可以直接走：
   - `python3 scripts/git_safety.py auto-closeout`
   - 需要真正一把推上去时再加 `--push`
6. 整理提交面：
   - 把真正源码改动和缓存/日志/临时文件分开
   - 保留用户无关改动，不要误吞
7. 验证：
   - 运行最小但足够的测试、lint、build 或 smoke
   - 没法验证时要明确说明风险
8. 提交与分支收口：
   - 用清晰提交信息提交
   - 若工作在 topic/worktree 分支上，优先按照 `publish-plan` 或 `auto-closeout` 给出的 `--ff-only` 路线收口
9. 推送：
   - 推送前确认 upstream、ahead/behind、remote 目标
   - 用显式 remote 和 branch；不要盲推

## Hard constraints

- 不要覆盖或丢弃用户未授权的改动
- 不要默认使用破坏性 Git 命令
- 不要把“整理”理解成偷偷删东西
- 不要在分叉、冲突、落后上游这些高风险情况下假装可以全自动
- 如果 worktree / stash / hooksPath 很可疑，先处理这些面，再谈“为什么改动被吞”

## Usage

```text
$gitx
```
