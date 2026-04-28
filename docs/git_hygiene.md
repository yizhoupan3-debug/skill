# Git Hygiene

这个仓库是高频变更仓库，而且经常同时存在主工作区、agent worktree、自动生成产物三条线。Git 操作以 Git 原生命令为准，先把真实状态看清楚。

## 日常基线

```bash
git status --short --branch
git worktree list --porcelain
git stash list
git diff --stat
git diff --cached --stat
```

这组命令能确认当前分支、upstream ahead/behind、未暂存改动、暂存区、stash 和 worktree 状态。

## 手动 Checkpoint

在清理、rebase、stash、切分提交前，先把恢复锚点写到 `artifacts/ops/`：

```bash
mkdir -p artifacts/ops/git-checkpoint
git status --short --branch > artifacts/ops/git-checkpoint/status.txt
git worktree list --porcelain > artifacts/ops/git-checkpoint/worktrees.txt
git stash list > artifacts/ops/git-checkpoint/stash.txt
git diff > artifacts/ops/git-checkpoint/tracked.patch
git diff --staged > artifacts/ops/git-checkpoint/staged.patch
```

未跟踪文件如需备份，先用 `git ls-files --others --exclude-standard` 列清单，再按需复制或打包。

## 主分支切片

禁止默认主动切分分支或创建 worktree。即使脏改堆在 `main`，也只能先做只读检查和必要 checkpoint；如果确实需要 topic 分支或 worktree 来隔离风险，先停下说明原因并等待用户明确要求。

只有用户主动要求创建分支时，才使用显式命令：

```bash
git switch -c topic/<task-name>
```

只有用户主动要求创建 worktree 时，才使用显式命令：

```bash
git worktree add <path> <branch-or-commit>
```

提交、merge、push 都要串行执行。推送前确认 upstream、远端名和目标分支，不要盲推。
