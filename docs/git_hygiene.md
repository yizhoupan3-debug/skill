# Git Hygiene

这个仓库是高频变更仓库，而且经常同时存在主工作区、agent worktree、自动生成产物三条线。只靠 `git status` 很容易误判“改动被吞了”，实际常见情况是：

- 改动还在，只是被 `stash`、旁路 worktree、或未检查的本地提交里藏住了
- 生成文件把提交范围扩大了，导致你对“这次到底改了什么”失去把握
- 长时间直接在 `main` 上堆脏改动，后面再清理成本很高

## 日常基线

先看总览：

```bash
python3 scripts/git_safety.py status
```

这会直接告诉你：

- 当前 `HEAD` / 分支 / upstream ahead-behind
- tracked、staged、untracked、ignored 数量
- 有几个 stash
- 有几个 worktree，以及它们是不是还停在旧 HEAD
- 当前 hooksPath 是不是和仓库维护预期一致

在任何清理、rebase、stash、切分提交前，先打一个无侵入 checkpoint：

```bash
python3 scripts/git_safety.py checkpoint --label before-cleanup
```

输出会落在 `artifacts/ops/git_safety/<timestamp>_before-cleanup/`，里面包含：

- `metadata.json`：分支、HEAD、stash、worktree、hooks 等摘要
- `tracked.patch`：未暂存 tracked 改动
- `staged.patch`：暂存区改动
- `untracked.tar.gz`：未跟踪文件备份
- `reflog.txt`：最近 HEAD 移动历史
- `RESTORE.md`：恢复顺序

这一步不会改写你的工作区，只是做恢复锚点。

## 推荐工作流

1. `python3 scripts/git_safety.py status`
2. `python3 scripts/git_safety.py checkpoint --label <task>`
3. 再做 `stash` / `rebase` / `checkout` / 切分提交
4. 如果状态不对，先看 checkpoint 目录里的 `metadata.json`、`reflog.txt`、`stash.txt`

## 主分支切片

如果你已经在 `main` 上堆了脏改，不要先手动 `stash`。先直接跑：

```bash
python3 scripts/git_safety.py start-topic topic/<task-name>
```

它会先自动写一个 checkpoint，再执行 `git switch -c topic/<task-name>`，把当前脏改原样带到新分支上。这样你至少同时有：

- 一个可恢复的 checkpoint
- 一个承接当前改动的新 topic 分支

这比“先 stash，过几小时再想起来恢复”稳定得多。

## 恢复顺序

在 checkpoint 目录里按这个顺序恢复：

1. `git apply --index staged.patch`
2. `git apply tracked.patch`
3. `tar -xzf untracked.tar.gz -C .`

以上命令都要在仓库根目录执行。
