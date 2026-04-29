# Codex Agent Policy

- 本文件所在目录就是 policy root；不要用 shell 当前目录推断 skill 根。
- 先查 `<policy-root>/skills/SKILL_ROUTING_RUNTIME.json`。
- 命中后只读对应的 `<policy-root>/skills/<name>/SKILL.md`。
- 不要预读整个 `skills/` skill 库。
- 用户要求 review、代码审核、PR review、实现质量检查、回归风险检查时，默认视为用户已明确要求开启一个独立上下文 reviewer subagent：使用 `fork_context=false`，只传必要仓库路径、diff/文件范围和审查目标；主线程可并行做轻量核对与最终汇总。若用户明确说不要 subagent、只要本地 review，才不要开启。
- 未经用户主动明确要求，不得主动创建 Git 分支或 Git worktree；也不要把“保持主线干净”“并行开发”“隔离风险”当作默认理由来创建。只允许只读检查现有分支/worktree 状态，若确实需要新分支或 worktree，先停下并询问用户。
