---
name: gitx
description: |
  Codex 里的 Git 主入口。Use when the user says `/gitx` or natural `gitx`, or
  needs branch、merge、rebase、push、worktree、仓库收口、推送失败排查等 Git 实操。
  This skill owns practical Git work in this repo, from quick diagnosis to end-to-end closeout.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Run the safe Git review-fix-tidy-commit-branch-merge-push workflow end to end.
trigger_hints:
  - /gitx
  - gitx
  - git 一条龙
  - review 修复 整理 提交 推送
  - 合并分支
  - merge branch
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

`gitx` 是给 Codex 用的 Git 收口快捷入口。推荐显式入口：`/gitx`（不再使用 `$gitx`）。

## When to use

- 用户明确说 `/gitx` 或口语 `gitx`
- 用户要查分支、合并分支、rebase、push、远程、worktree、stash、仓库状态这类 Git 实操
- 用户要把 review、修复、整理、提交、合并分支、合并 worktree、推送当成一次连续动作做完
- 当前重点是把仓库安全收口，或把 Git 问题落到真实仓库状态上处理

## Do not use

- 只是要一个通用 shell 命令/管道，而不是 Git 协作或仓库状态问题 -> 直接在当前上下文回答或执行
- 根因还不清楚，先要查为什么坏了 -> `$systematic-debugging`
- 只是做 PR 评论收口 -> `$gh-address-comments`
- 只是做纯代码 review，不涉及提交流程 -> 使用普通 code review 输出，不走 Git 收口入口

## Default contract

把 **`/gitx`** 视为用户对当前仓库发出的“安全一条龙收口”授权，默认目标是：

1. 先看清真实 Git 状态，而不是直接提交
2. 先 review 再 fix
3. 先整理脏改动和 worktree，再决定怎么提交或合并分支
4. 最后把应该推送的分支安全推上去（默认直接执行，不再二次询问）

如果当前目录不是 Git 仓库，不要擅自初始化；直接说明不是仓库并停下。

## 入口语义：空 `/gitx` vs 带参

- **仅入口词、无后续说明**（行内只有 `/gitx` 或口语 `gitx`，或去掉首尾空白后不再有其它文字）：视为 **全工作区 Git 收口**——把当前仓库当作一个整体：所有已跟踪改动、暂存区、未跟踪里与本次收口相关的文件、stash、worktree 关系、当前分支与 upstream 等，按下文 **Required workflow** 一条龙处理到安全状态（整理、提交/拆分提交、merge、push 等）；**「弄干净」指有序收口而非擅自 `git clean -fd` 等破坏性清理**。
- **入口词后面还有内容**（例如 `/gitx scripts/router-rs`、`/gitx 只处理 AGENTS.md`）：视为 **范围限定**——仍以全仓诊断命令看清全局，但 **review、修复、拆分/整理提交面、验证与最终纳入提交的 path** 只针对用户给出的范围（路径、目录、模块、分支名或一句明确议题）；**不要把明显无关路径的改动顺手塞进同一提交**，除非为消除该范围内的构建/测试失败所必需。

## Review 深度（是否「先深度 review」）

- **不是**默认第一步做全仓深度代码审阅；默认顺序是：**诊断 → 提交面/范围摸底 → 对拟纳入收口的改动做务实 review**（明显 bug、回归、脏文件、生成物、遗漏测试等），再 fix 与验证。
- 宿主若对 `/gitx` 启用并行 reviewer lane，属于执行面上的评审分路，**不改变**上述「先看清 Git 状态与范围，再审改动面」的顺序。

## Execution tiers

- 只诊断：`git status --short --branch` + `git worktree list --porcelain`
- 看提交面：`git diff --stat` + `git diff --cached --stat`
- 看远端关系：`git rev-parse --abbrev-ref --symbolic-full-name @{u}` + `git status --short --branch`
- 验证：直接运行本次改动面需要的 `cargo test` / `pytest` / `npm test` / smoke 命令
- 收口：自动执行安全分支、提交、merge、push 路线；不要依赖已移除的 Python git helper

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
7. 自动化覆盖低风险分支合并：
   - 目标分支和源分支可由用户请求、当前分支、upstream 或 worktree 上下文明确推断时，可以继续执行
   - merge 前确认 `git status --short --branch` 干净或只有本次已纳入提交面的改动
   - 优先 `git merge --ff-only <source>`；需要 merge commit 时只在用户明确允许或仓库惯例明确时执行
   - 遇到冲突、落后远端、分叉历史、未跟踪文件覆盖风险、目标/源分支不清楚时停止并说明下一步
8. 低风险场景也要显式记录提交面、源分支、目标分支和远端，不走隐藏状态。
9. 整理提交面：
   - 把真正源码改动和缓存/日志/临时文件分开
   - 保留用户无关改动，不要误吞
10. 验证：
   - 运行最小但足够的测试、lint、build 或 smoke
   - 没法验证时要明确说明风险
11. 提交与分支收口：
   - 用清晰提交信息提交
   - 若工作在 topic/worktree 分支上，优先用 `git merge --ff-only` 路线合并回目标分支
12. 推送：
   - 推送前确认 upstream、ahead/behind、remote 目标
   - 用显式 remote 和 branch；不要盲推
   - 默认在收口完成后立即推送，不需要再次征求用户确认
   - 仅在高风险阻塞场景暂停推送并说明原因：冲突、非快进拒绝、分叉历史不清、目标远端不明确、或权限/认证失败

## Hard constraints

- 不要覆盖或丢弃用户未授权的改动
- 不要默认使用破坏性 Git 命令
- 不要把“整理”理解成偷偷删东西
- 不要在分叉、冲突、落后上游这些高风险情况下假装可以全自动
- 如果 worktree / stash / hooksPath 很可疑，先处理这些面，再谈“为什么改动被吞”

## Usage

```text
/gitx
/gitx <路径、目录、分支或一句明确范围>
```
