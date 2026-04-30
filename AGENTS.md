# Codex Agent Policy

## Scope

- 本文件所在目录就是 policy root。
- 不要用 shell 当前目录推断 skill 根。

## Routing

- 先查 `<policy-root>/skills/SKILL_ROUTING_RUNTIME.json`。
- 命中 skill 后，只读 runtime 记录里的 `skill_path` 对应文件。
- `skill_path` 按 `<policy-root>/<skill_path>` 解析，不要用 slug 猜路径。
- runtime 未命中且确需继续路由时，才查 fallback manifest。
- 不要预读整个 `skills/` skill 库。

## Delegation

- 默认主线程负责交付；subagent 是边车，不是默认执行模式。
- Review、深度调研、全仓/跨模块排查、多方向方案、并行检查、多文件独立实现、多个独立假设验证，或用户说“同时 / 分头 / 分路 / 并行 / 多方向 / 多模块”时，先做 bounded sidecar admission。
- 只有同时满足这些条件才 spawn：lane 独立、范围明确、可验证、不阻塞主线程下一步、上下文收益大于协调成本。
- 适合 spawn 的典型 lane：高噪音搜索、日志/测试输出整理、独立模块调研、独立风险审查、互不重叠的文件级实现。
- 不适合 spawn 的情况：小任务、强共享上下文、顺序依赖、同文件/同模块写入、验证方式不清、用户明确要求本地处理。
- 符合条件时开启 1-3 个 subagent，并优先在同一轮并发启动；否则主线程直接做，并简短记录不 spawn 的原因。
- Subagent 默认 `fork_context=false`，只传仓库路径、相关文件/diff、lane 目标、禁止范围、输出契约和验证要求。
- 写入型 worker 只能负责明确 disjoint 的文件或模块，且不得修改共享连续性 artifact。
- 主线程始终保留即时阻塞项、集成判断和最终验证。
- 只有用户显式调用 `$team` / `/team`，或任务需要 worker 之间直接协作、共享任务列表、相互质询时，才升级到 team orchestration。

## Git

- 未经用户主动明确要求，不得主动创建 Git 分支或 Git worktree。
- 不要把“保持主线干净”“并行开发”“隔离风险”当作默认创建分支或 worktree 的理由。
- 只允许只读检查现有分支/worktree 状态。
- 若确实需要新分支或 worktree，先停下并询问用户。
