# Claude Shared Memory Projection

_Generated from shared runtime artifacts and `./.codex/memory/`. Do not edit manually._

- generated_at: 2026-04-19T02:15:39+08:00
- repo_root: `/Users/joe/Documents/skill`

## Recent Completed Task

- task: Deep audit the current routing system and close verified drift in trace metadata versioning.
- phase: completed
- status: completed
- route: subagent-delegation / skill-developer-codex / execution-audit-codex
- terminal_reasons: summary phase is terminal: completed / summary status is terminal: completed / supervisor phase is terminal: completed / verification status is terminal: completed / continuity story_state is terminal: completed
- follow_up_notes: Watch for any other trace producers that bypass scripts/write_trace_metadata.py and require the same runtime-version contract. / If you want a broader policy cleanup pass, tighten entry docs around @RTK.md and other operator-facing references next.
- current_execution_injection: blocked

## Stable Project Patterns

- **AP-1: 图片路径先转 ASCII** — 仓库根路径含中文，聊天贴图前先转存 ASCII-only 路径
- **AP-2: Skill 变更后必跑 sync+check** — 修改任何 `SKILL.md` 后必须 `python3 scripts/sync_skills.py --apply` 并执行 `python3 scripts/check_skills.py --verify-sync`
- **AP-3: 复杂任务先外置状态** — 复杂执行优先把状态写入 `SESSION_SUMMARY / NEXT_ACTIONS / EVIDENCE_INDEX / TRACE_METADATA / .supervisor_state`
- **AP-4: 自动化固定范围优先** — 自动化只适合固定范围、低歧义、可验证的维护动作
- **AP-5: 敏感动作前先确认** — 外部发送、公共发布、账号操作前默认确认；用户明确要求“直接执行”时可跳过

## Stable Decisions

- 项目长期记忆固定落在 `<workspace>/.codex/memory/`
- `./.codex/` 是共享框架内存根，不代表仅限 Codex 宿主
- 身份记忆固定落在 `~/.codex/identity/`
- framework bootstrap 只读取共享闭环产物，不再依赖旧宿主目录或注入链路
- 当前任务真实状态始终以 task artifacts 与 `.supervisor_state.json` 为准
- 复杂任务默认先走 `execution-controller-coding`

## Recent Lessons

- **L-1**: transport TRACE 日志会迅速膨胀，不能默认长期保留
- **L-2**: 可重建缓存不应混入长期记忆层
- **L-3**: 记忆 consolidation 要优先提炼稳定结论，而不是复制原始上下文

## Artifact Anchors

- root task mirror: `/Users/joe/Documents/skill/.supervisor_state.json`
- `SESSION_SUMMARY.md`
- `NEXT_ACTIONS.json`
- `EVIDENCE_INDEX.json`
- `TRACE_METADATA.json`
- active task pointer: `/Users/joe/Documents/skill/artifacts/current/active_task.json`
- current session mirror: `artifacts/current/SESSION_SUMMARY.md`
- `artifacts/current/SESSION_SUMMARY.md`
- `artifacts/current/NEXT_ACTIONS.json`
- `artifacts/current/EVIDENCE_INDEX.json`
- `artifacts/current/TRACE_METADATA.json`
- `artifacts/current/<task_id>/`
- `./.codex/memory/`
- logical->physical memory mapping: `./.codex/memory/` -> `memory`
- sync rule: Supervisor writes task-scoped continuity under artifacts/current/<task_id>/ and keeps root plus artifacts/current compatibility mirrors aligned to the same task. artifacts/current/ should contain only the active-task pointer, four mirror files, and task-scoped continuity directories; bootstrap, ops, evidence, and scratch belong elsewhere.
