---
allowed_tools:
- shell
- git
- python
approval_required_tools: []
description: Run the safe Git review-fix-tidy-commit-branch-merge workflow end to end (commit/merge only).
metadata:
  platforms:
  - supported
  tags:
  - git
  - git-closeout
  - review
  - commit
  - worktree
  version: '1.4.0'
name: gitx
scene: general
network_access: conditional
risk: medium
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P1
session_start: n/a
short_description: Run the Git closeout workflow with deep review on the substantive diff before commit/merge.
source: runtime
trigger_hints:
- /gitx
- gitx
- 提交代码
- 代码提交
- commit
- git commit
- 合并
- merge
- 提交
- 分支合并
- worktree 合并
- 清理分支
- 收拢分支
---
# gitx — Git 自主收口

三条车道，按改动规模自动匹配。全流程自主执行，仅在安全边界处暂停。

## 三车道模型

| 车道 | 判断方式 | 流程 |
|------|---------|------|
| **快车道** | Claude 看 diff 判定仅 typo/文档/format/重命名，**无逻辑或配置改动** | status → `diff --stat` + `diff --check` → commit，skip 深度 review |
| **标准车道** | 含逻辑/配置/测试/API 改动 | 诊断 → 深度 review → fix → 验证 → commit |
| **清理车道** | 用户明确说 rebase / amend / squash / 整理历史 | 只整理已有 commits，不经新代码 review |

快车道不由行数/文件数硬编码。Claude 看改动面自行判断；**发现逻辑改动混入时自动降级为标准车道**。

## 入口语义

- **`/gitx`** — 自动判断车道
- **`/gitx <路径/目录>`** — 限定范围，只收口该范围内的改动
- **`/gitx rebase`** 或 **`/gitx squash`** — 强制走清理车道

## 统一工作流

### 1. 诊断（所有车道）

```bash
git status --short --branch
git worktree list --porcelain
git stash list
```

**异常自动处理：**
- **dirty 在 `main` 上** → 正常在 main 操作（gitx 设计约定），不创建分支
- **有 stash** → 若 stash 数量少（≤3）且内容与当前工作相关，自动 pop/apply；量大或无关则跳过并报告 stash 状态
- **worktree 头部不一致** → 记录状态，继续当前 worktree 的操作

### 2. 车道判定

Claude 根据 diff 内容自动选择车道，无需用户确认。仅在以下情况暂停说明：
- 快车道发现逻辑改动混入 → 自动降级
- 无法判定改动类型 → 列明原因

### 3. 厘清提交面（快车道 skip）

```bash
git diff --stat
git diff --cached --stat
git diff --check          # 预检空白符/冲突标记
```

- 多个独立议题 → Claude 依据文件调用链判断最合理的分组边界，不暂停问用户；只有改动明显不相干时才说明分组逻辑
- 自动 `git add -p` 拆分明显无关的生成物/缓存文件
- 对生成物文件 `git restore --staged` 或追加 `.gitignore`

### 4. 深度 review checklist（标准车道）

逐项落实，发现问题自动 fix 再继续。不暂停问用户：

1. **Substantive diff** — 读完整 `git diff`，不止 `--stat`；确认无调试残留、误改生成物、意外敏感信息
2. **回归向量** — 对改动面相称的最小充分测试
3. **风险收口** — 跨界改动核对 `AGENTS.md` / runtime 真源
4. **验证记录** — 收口说明带通过的命令摘要

**可疑代码**：`git blame -L <start>,<end> <file>` 查引入时间；若发现严重问题（硬编码密钥、安全漏洞）则暂停并说明。

### 5. 验证（标准车道）

```bash
# 自动检测项目类型
```

检测策略（按顺序匹配，自动执行）：
- 存在 `Cargo.toml` → `cargo test --quiet && cargo clippy --quiet -D warnings`
- 存在 `pytest.ini` / `pyproject.toml` → `pytest -q`
- 存在 `package.json` → `npm test 2>&1 | tail -20`
- 存在 `Makefile` → `make test`
- 都不匹配 → 直接跳过，说明无法自动验证

### 6. 提交（自主执行）

展示以下信息后**直接执行**，不等待用户确认：

```bash
# Claude 展示并执行
git diff --stat
git diff --check
git commit -m "type(scope): 简述

- 关键改动列表
- 动机（非明显时）
"
```

commit message 格式：
- 快车道：`type(scope): 一行简短描述`
- 标准车道：**主题行 + body**（具体改了啥 + 动机）

**提交后**：直接 main 上已完成。若过程中遇到冲突、`git merge --ff-only` 拒绝则**暂停并说明**。

### 7. 清理车道

- 展示拟操作范围（`git log --oneline HEAD~N..HEAD`）
- 说明计划（reword / squash / drop / 拆分）
- **rebase 改写历史，必须暂停等用户显式确认**
- 确认后执行并 `git range-diff` 验证

### 8. 分支/worktree 收拢

用户要求「合并分支到 main」「合并 worktree 到 main」「清理分支/worktree」时执行。全自动化执行安全操作：

```bash
git branch
git worktree list --porcelain
```

遍历非 `main` 分支：

| 条件 | 行为 |
|------|------|
| `git merge --ff-only` 成功 | 自动合并 + `git branch -d` + 清理对应 worktree |
| 分叉不可快进 | 跳过，记录分支名和 ahead/behind 数，说明原因 |
| 已 merged 到 main | 自动 `git branch -d` + 清理 worktree |

**暂停条件**（仅在这些情况下停止并说明，不自作主张）：
- `git merge --ff-only` 因冲突拒绝
- 工作目录不干净
- 目标分支不可推导
- 远端 tracking 分支需要手动处理

## Verification tiers

| 检测条件 | 命令 |
|----------|------|
| `Cargo.toml` 存在 | `cargo test --quiet && cargo clippy --quiet -D warnings` |
| `pytest.ini` / `pyproject.toml` 存在 | `pytest -q` |
| `package.json` 存在 | `npm test 2>&1 \| tail -20` |
| `Makefile` 存在 | `make test` |
| 框架仓策略层 | 追加 `cargo test policy_contracts` |
| 通用 diff 预检 | `git diff --check` |

## Hard constraints

- gitx **自主执行，不等待用户确认**，但以下情况必须暂停并说明：
  - `rebase` / `commit --amend` 改写历史
  - `git merge` 遇到冲突或分叉历史
  - 发现安全敏感信息（密钥、凭证）
  - 无法推断操作目标（分支、远端）
- gitx **不创建 topic 分支**，所有改动直接在 `main` 上操作
- 不要默认使用破坏性命令（`git clean -fd` / `git reset --hard` 等）
- 分支/worktree 收拢时只删除已快进合并到 `main` 的分支，分叉分支仅报告
- 远端分支只读不删

## Usage

```text
/gitx                        # 自动判断车道
/gitx <路径>                  # 限定范围收口
/gitx rebase                 # 强制走清理车道（须确认）
/gitx squash HEAD~3          # 合并最近 3 条提交（须确认）
/gitx merge <branch>         # 将指定分支合并到 main + 删除
/gitx cleanup-worktrees      # 收拢 worktree 到 main + 删除冗余
```
