# Codex Agent Policy

- 本文件所在目录就是 policy root；不要用 shell 当前目录推断 skill 根。
- 先查 `<policy-root>/skills/SKILL_ROUTING_RUNTIME.json`。
- 命中后只读对应的 `<policy-root>/skills/<name>/SKILL.md`。
- 不要预读整个 `skills/` skill 库。
- 用户要求 review、代码审核、PR review、实现质量检查、回归风险检查时，默认视为用户已明确要求开启一个独立上下文 reviewer subagent：使用 `fork_context=false`，只传必要仓库路径、diff/文件范围和审查目标；主线程可并行做轻量核对与最终汇总。若用户明确说不要 subagent、只要本地 review，才不要开启。
- 用户要求深度调研、全仓/跨模块排查、多方向方案、并行检查、多文件独立实现、多个独立假设验证，或使用“同时 / 分头 / 分路 / 并行 / 多方向 / 多模块”这类表达时，默认视为用户已明确要求先做 bounded sidecar admission：能拆出独立、可验证、非阻塞 lane 时，主动开启 1-3 个 subagent，优先在同一轮并发启动。
- Codex subagent 默认使用 `fork_context=false`，只传必要仓库路径、diff/文件范围、lane 目标、禁止范围和输出契约；主线程保留即时阻塞项、集成判断和最终验证。只有在任务很小、共享上下文过重、写入范围重叠、下一步被该结果阻塞、验证方式缺失或协调成本明显更高时才不 spawn，并在主线程简短记录拒绝原因。
- 多个 subagent 的 lane 必须互不重叠：read-only 调研/审查可以按模块、假设、风险面或验证面拆；写入型 worker 只能负责明确 disjoint 的文件或模块，且不得修改共享连续性 artifact。
- 未经用户主动明确要求，不得主动创建 Git 分支或 Git worktree；也不要把“保持主线干净”“并行开发”“隔离风险”当作默认理由来创建。只允许只读检查现有分支/worktree 状态，若确实需要新分支或 worktree，先停下并询问用户。
