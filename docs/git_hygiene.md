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

如果脏改堆在 `main`，先做 checkpoint，再显式创建 topic 分支：

```bash
git switch -c topic/<task-name>
```

提交、merge、push 都要串行执行。推送前确认 upstream、远端名和目标分支，不要盲推。
