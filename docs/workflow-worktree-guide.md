# Workflow Worktree 指南

## 背景

Claude Code 的 Workflow 运行时会在 `agent()` 调用时自动创建 git worktree 隔离环境。
这意味着 agent 对文件的修改发生在 worktree 中，**而非主工作区**。

## 核心规则

### 1. 修改文件的 agent 必须 commit

在 workflow 脚本的 `agent()` prompt 中，如果 agent 会修改文件（Edit/Write 操作），
必须在 prompt 末尾要求：

```
完成后执行 git add <修改的文件> && git commit -m "<描述>"
```

commit 后的 worktree 在 workflow 结束时会被正常清理。未 commit 的 worktree 会残留。

### 2. 脚本顶层文件操作必须手动 commit

如果 workflow 脚本自身（非 agent 内）通过 `writeFileSync` 等写文件，
需要在写入后手动执行 git commit：

```js
const { execSync } = await import('child_process')
execSync('git add "<path>" && git commit -m "<msg>" --no-verify', {
  cwd: '<repo_root>',
  timeout: 10000
})
```

参考：`claude-code-cli-audit.js` 中的实现。

### 3. 只读 agent 无需 commit

返回 JSON schema 的审计/分析 agent（只做 Read/grep）不需要 commit。
它们的 worktree 在无修改时会被自动清理。

### 4. worktree 残留清理

如果发现 `.claude/worktrees/` 下有残留 worktree：

```bash
# 查看残留
git worktree list

# 强制清理（丢弃未提交修改）
for d in .claude/worktrees/wf_*/; do
  git worktree remove --force "$d" 2>/dev/null
done

# 清理残留分支
git branch --list 'worktree-wf_*' | xargs git branch -D

# 清理 stale lock（进程已退出但锁未释放）
for d in .claude/worktrees/wf_*/; do
  git worktree remove -f -f "$d" 2>/dev/null
done
```

## session_launch worktree 支持

通过 `session_launch` MCP 工具启动 session 时，可以指定 worktree：

- `worktreeName`：worktree 分支名，supervisor 会在 `.claude/worktrees/<name>` 创建
- `worktreePath`：显式 worktree 路径，优先于 worktreeName

session supervisor 会自动将 session 的工作目录切换到 worktree 路径。
