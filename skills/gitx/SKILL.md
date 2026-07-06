---
allowed_tools:
- Bash
- Read
- Write
- Edit
approval_required_tools: []
description: 'Git 自主收口：review-fix-tidy-commit-branch-merge 全流程，三车道自动匹配'
kind: skill
metadata:
  platforms:
  - supported
  tags:
  - git
  - commit
  - merge
  - worktree
  - closeout
  version: '2.1.0'
name: gitx
scene: general
network_access: conditional
risk: medium
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P1
session_start: n/a
short_description: 'Git 自主收口——review-fix-tidy-commit-branch-merge 全流程'
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
- 收口
- 帮我提交
- 提交一下
- 提交改动
- 代码收尾
- git收尾
- git 收尾
- 整理提交
- 代码整理
- close out
- wrap up
- tidy up
- 提交吧
- 提交了
- 提交上去
- 推上去
when_to_use: '用户要求提交代码、收口改动、合并分支、清理 worktree、整理 git 历史时使用。关键词：提交、commit、收口、merge、rebase、squash、清理分支、worktree。自然语言变体：「帮我提交」「提交一下」「收口吧」「代码整理」「close out」。'
do_not_use: '不用于查看 git log、git diff 只读浏览、创建新分支（非合并）、git blame 调查。不用于普通代码 review（用 code-review-deep）。'
---
# gitx — Git 自主收口

三条车道，按改动规模自动匹配。全流程自主执行，仅在安全边界处暂停。

## 三车道模型

| 车道 | 判断方式 | 流程 |
|------|---------|------|
| **快车道** | 仅 typo/文档/format/重命名，**无逻辑或配置改动** | 诊断 → commit |
| **标准车道** | 含逻辑/配置/测试/API 改动 | 诊断 → 深度 review → fix → 验证 → commit |
| **清理车道** | 用户明确说 rebase / amend / squash / 整理历史 | 只整理已有 commits |

快车道不由行数/文件数硬编码。Claude 看改动面自行判断；**发现逻辑改动混入时自动降级为标准车道**。

## 入口语义

| 用法 | 行为 |
|------|------|
| `/gitx` | 自动判断车道 |
| `/gitx <路径/目录>` | 限定范围，只收口该范围内的改动 |
| `/gitx rebase` / `/gitx squash` | 强制走清理车道 |
| `/gitx squash HEAD~3` | 合并最近 N 条提交（须确认） |
| `/gitx merge <branch>` | 将指定分支合并到 main + 删除 |
| `/gitx cleanup-worktrees` | 收拢 worktree 到 main + 删除冗余 |

## 统一工作流

### 1. 前置状态检查（所有车道）

**必须最先执行，任何异常自动处理：**

```bash
git status --short --branch
git worktree list --porcelain
git branch --merged main
git diff --stat
git diff --check
```

**异常自动处理（不暂停）：**

| 异常 | 处理 |
|------|------|
| rebase / merge / cherry-pick 进行中 | 记录状态，提示用户先完成或 abort，**暂停退出 gitx** |
| detached HEAD | 记录 HEAD 指向，提示用户切回分支，**暂停退出 gitx** |
| dirty 在 `main` 上 | 正常操作，不创建分支 |
| worktree 头部不一致 | 记录状态，继续当前 worktree |
| 已 merged 分支 | 标记待清理，§8 自动执行 |
| untracked 文件 | 按文件类型和 .gitignore 自动 `git add`，生成物/缓存跳过 |
| 空白符/冲突标记 | 自动修复（trailing whitespace），冲突标记标记为 conflict |

### 2. 车道判定

Claude 根据 diff 内容自动选择车道，无需用户确认。**自动降级规则**：
- 快车道发现逻辑改动混入 → 自动降级为标准车道
- 无法判定改动类型 → 默认走标准车道

### 3. 厘清提交面

```bash
git diff --stat
git diff --cached --stat
git diff --check
```

- 多个独立议题 → Claude 依据文件调用链自动分组，不暂停
- 自动 `git add` 合理文件，跳过生成物/缓存/临时文件
- 对误加的生成物自动 `git restore --staged`

### 4. 深度 review checklist（标准车道）

逐项落实，发现问题自动 fix，不暂停：

1. **Substantive diff** — 读完整 `git diff`；确认无调试残留、误改生成物、意外敏感信息
2. **安全扫描** — 检查硬编码密钥、凭证、API token；发现则**暂停并说明**
3. **回归向量** — 自动运行改动面关联的测试
4. **风险收口** — 跨界改动核对 AGENTS.md / runtime 真源
5. **验证记录** — 自动收集通过的命令摘要

**可疑代码**：`git blame -L <start>,<end> <file>` 查引入时间。

### 5. 验证（标准车道，全自动化）

按检测策略自动匹配并执行，不暂停：

| 优先级 | 条件 | 命令 |
|--------|------|------|
| 1 | `Cargo.toml` 存在 | `cargo test --quiet && cargo clippy --quiet -D warnings` |
| 2 | `pytest.ini` / `pyproject.toml` 存在 | `pytest -q` |
| 3 | `package.json` 存在 | `npm test 2>&1 \| tail -20` |
| 4 | `Makefile` 存在 | `make test` |

不匹配时跳过。

**验证失败处理**：自动 fix 后重试一次；仍失败则暂停说明，**不提交未通过验证的代码**。

### 6. 提交（自主执行，全自动化）

**提交前安全检查**：
```bash
git diff --cached --stat          # 确认 staged 内容
git diff --cached --check         # 确认无空白符问题
```

确认无误后直接执行：
```bash
git commit -m "type(scope): 简述

- 关键改动列表
- 动机（非明显时）
"
```

**commit message 自动推断**：
- scope 从改动文件路径推断（如 `core/skill-layer` → `skill-layer`）
- type 从改动性质推断：fix/feat/refactor/docs/test/chore
- 快车道：一行简短描述
- 标准车道：主题行 + body

**提交后**：
```bash
git log --oneline -1              # 展示提交哈希
git status --short                # 确认工作区干净
```

**提交失败处理**：暂停说明原因，不重试（避免重复提交）。

### 7. 推送（自动执行）

提交成功后自动执行，不暂停：

```bash
git remote -v                                   # 检查是否有 upstream remote
git rev-list --count --left-right main...@{upstream}  # ahead/behind
```

**有 upstream remote 且 ahead > 0** → 直接 `git push`

**异常处理（全自动，不暂停）**：

| 异常 | 处理 |
|------|------|
| 无 remote | 静默跳过 |
| push rejected（non-fast-forward） | 自动 `git pull --ff-only` → 再次 `git push` → 仍失败则静默跳过 |
| 无 ahead（remote 已最新） | 静默跳过 |

### 8. 清理车道

```bash
git log --oneline HEAD~N..HEAD
```

- 说明计划（reword / squash / drop / 拆分）
- **rebase 改写历史必须暂停等用户确认**
- 确认后执行，`git range-diff` 验证
- **rebase 失败**：自动 `git rebase --abort`，暂停说明

### 9. 分支/worktree 收拢（每次 gitx 自动执行）

提交完成后自动执行，不暂停：

```bash
git branch --merged main | grep -v '^\*' | grep -v 'main'
git worktree list --porcelain
```

遍历非 main 分支：

| 条件 | 行为 |
|------|------|
| 已 merged 到 main | 自动 `git branch -d` + 清理对应 worktree |
| `git merge --ff-only` 成功 | 自动合并 + `git branch -d` + 清理 |
| 分叉不可快进 | 跳过，记录分支名和 ahead/behind 数 |

**唯一暂停条件**：`git merge --ff-only` 因冲突拒绝。

## 错误恢复总则

| 步骤 | 失败时 |
|------|--------|
| 诊断 | 遇到 rebase/merge/detached HEAD → 暂停退出 |
| review | 发现安全问题 → 暂停说明 |
| 验证 | 测试失败 → 自动 fix 重试一次；仍失败则暂停 |
| 提交 | commit 失败 → 暂停说明，不重试 |
| 推送 | push rejected → `git pull --ff-only` 重试 → 仍失败则静默跳过（不暂停） |
| rebase | 冲突 → `git rebase --abort`，暂停说明 |
| 收拢 | ff-only 冲突 → 跳过该分支，继续其他 |

## Hard constraints

- gitx **全自动化执行，不等待用户确认**，仅以下情况暂停：
  - `rebase` / `commit --amend` 改写历史
  - `git merge` 遇到冲突或分叉历史
  - 发现安全敏感信息（密钥、凭证）
  - rebase / merge / cherry-pick 进行中
  - detached HEAD
- gitx **不创建 stash**，所有改动直接在工作区操作
- gitx **不创建 topic 分支**，所有改动直接在 main 上操作
- 不使用破坏性命令（`git clean -fd` / `git reset --hard` 等）
- 远端分支只读不删
- **不提交未通过验证的代码**

## Usage

```text
/gitx                        # 自动判断车道
/gitx <路径>                  # 限定范围收口
/gitx rebase                 # 强制走清理车道（须确认）
/gitx squash HEAD~3          # 合并最近 3 条提交（须确认）
/gitx merge <branch>         # 将指定分支合并到 main + 删除
/gitx cleanup-worktrees      # 收拢 worktree 到 main + 删除冗余
```
