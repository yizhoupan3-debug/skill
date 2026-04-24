---
name: gitx
description: |
  Codex 里的 Git 主入口。Use when the user says `$gitx` / `/gitx` / `gitx`, or
  needs branch、rebase、push、worktree、仓库收口、推送失败排查等 Git 实操。
  This skill owns practical Git work in this repo, from quick diagnosis to end-to-end closeout.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Run the safe Git review-fix-tidy-commit-merge-push workflow end to end.
trigger_hints:
  - $gitx
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
- 用户要查分支、rebase、push、远程、worktree、stash、仓库状态这类 Git 实操
- 用户要把 review、修复、整理、提交、合并 worktree、推送当成一次连续动作做完
- 当前重点是把仓库安全收口，或把 Git 问题落到真实仓库状态上处理

## Do not use

- 只是要一个通用 shell 命令/管道，而不是 Git 协作或仓库状态问题 -> `$shell-cli`
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

- 只诊断：`git status --short --branch` + `git worktree list --porcelain`
- 看提交面：`git diff --stat` + `git diff --cached --stat`
- 看远端关系：`git rev-parse --abbrev-ref --symbolic-full-name @{u}` + `git status --short --branch`
- 验证：直接运行本次改动面需要的 `cargo test` / `pytest` / `npm test` / smoke 命令
- 收口：人工明确分支、提交、merge、push 路线；不要依赖已移除的 Python git helper

## Required workflow

1. 先跑 gitx 诊断，而不是上来就提交：
   - `git status --short --branch`
   - `git worktree list --porcelain`
   - `git stash list`
2. 需要看原始面时，再补：
   - `git diff --stat`
   - `git diff --cached --stat`
   - `git status --short --branch`
3. 若发现脏改动直接堆在 `main`、存在 stash、或 worktree 头部不一致：
   - 先做手动 checkpoint：保存 `git diff`、`git diff --staged`、必要的 untracked 清单到 `artifacts/ops/`
   - 不要直接在混乱状态下提交
4. 对待提交改动做 review：
   - 先找明显 bug、回归、脏文件、生成物噪音、遗漏测试
   - 需要时先修复再继续
5. 能并行的面尽量放在收口前半段：
   - 只读审计可以并行看：status / worktree / stash / hooks / reflog
   - 验证命令可以并行跑，但提交、merge、push 必须串行
   - 真正改 Git 状态的临界区仍保持串行，不要并发提交、并发 merge
6. 高输出验证默认优先走 repo 里的 RTK 规则：
   - `cargo test` / `npm test` / `git diff` 这类噪声命令，允许自动加 `rtk`
   - 若需要原始输出，用 `--no-rtk`
7. 低风险场景也要显式确认提交面和目标分支，不走隐藏自动化。
8. 整理提交面：
   - 把真正源码改动和缓存/日志/临时文件分开
   - 保留用户无关改动，不要误吞
9. 验证：
   - 运行最小但足够的测试、lint、build 或 smoke
   - 没法验证时要明确说明风险
10. 提交与分支收口：
   - 用清晰提交信息提交
   - 若工作在 topic/worktree 分支上，优先用 `git merge --ff-only` 路线收口
11. 推送：
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
