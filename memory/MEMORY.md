# 项目长期记忆

_本文件只沉淀跨会话稳定的项目事实、决策与约定；当前任务态只看 `artifacts/current/<task_id>/`、`artifacts/current/active_task.json` 与 root `.supervisor_state.json`。_

## 项目身份

- **仓库**: `/Users/joe/Documents/skill`
- **核心关注**: skill 路由系统、执行编排、共享 framework bootstrap、三 CLI 治理、长期记忆与自动化
- **闭环事实源**: 稳定层在 `./.codex/memory/`；当前任务真源在 `artifacts/current/<task_id>/` + `artifacts/current/active_task.json` + `.supervisor_state.json`

## Active Patterns

_从重复验证的决策中提炼出的操作性约束。新会话启动时直接加载，无需回溯原始日志。_

- **AP-1: 图片路径先转 ASCII** — 仓库根路径含中文，聊天贴图前先转存 ASCII-only 路径
- **AP-2: Skill 变更后必跑 sync+check** — 修改任何 `SKILL.md` 后必须 `python3 scripts/sync_skills.py --apply` 并执行 `python3 scripts/check_skills.py --verify-sync`
- **AP-3: 复杂任务先外置状态** — 复杂执行优先把状态写入 `SESSION_SUMMARY / NEXT_ACTIONS / EVIDENCE_INDEX / TRACE_METADATA / .supervisor_state`
- **AP-4: 自动化固定范围优先** — 自动化只适合固定范围、低歧义、可验证的维护动作
- **AP-5: 敏感动作前先确认** — 外部发送、公共发布、账号操作前默认确认；用户明确要求“直接执行”时可跳过

## 稳定决策

### 共享 CLI 闭环记忆架构（2026-04-11）

- 项目长期记忆固定落在 `<workspace>/.codex/memory/`
- `./.codex/` 是共享框架内存根，不代表仅限 Codex 宿主
- 身份记忆固定落在 `~/.codex/identity/`
- framework bootstrap 只读取共享闭环产物，不再依赖旧宿主目录或注入链路
- 当前任务真实状态始终以 task artifacts 与 `.supervisor_state.json` 为准

### 执行编排（2026-04-11）

- 复杂任务默认先走 `execution-controller-coding`
- 非 trivial 会话必须记录 checkpoint，并在结束时执行 `cargo run --quiet --manifest-path scripts/router-rs/Cargo.toml -- --claude-hook-command session-end --repo-root <repo_root>`；兼容别名 `end-session`
- framework bootstrap 只 propose 上下文，Codex runtime 验证并 apply

### 多模型治理（2026-04-11）

- 当前三 CLI 主线为 Codex / Claude / Gemini，统一消费同一套 `skills/` 基础设施
- 其他模型宿主仍可通过同一框架闭环接入，但不再主导目录命名
- skill 路由以项目内 `skills/` 和 runtime artifacts 为准，不依赖外部旧宿主 skill 宿主

## 项目约定

- 仓库根路径含中文，图片嵌入必须先转存 ASCII 路径
- Skill 变更后必须执行 `sync_skills --apply` + `check_skills --verify-sync`
- 未经读取或验证的文件/行为，不写成既成事实

## Lessons

_从重复出现的错误模式中提炼。同一错误出现 ≥2 次后升级到这里。_

- **L-1**: transport TRACE 日志会迅速膨胀，不能默认长期保留
- **L-2**: 可重建缓存不应混入长期记忆层
- **L-3**: 记忆 consolidation 要优先提炼稳定结论，而不是复制原始上下文
