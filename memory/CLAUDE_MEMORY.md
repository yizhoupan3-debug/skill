# Claude Shared Memory Projection

_Generated from shared runtime artifacts and `./.codex/memory/`. Do not edit manually._

- generated_at: 2026-04-17T00:27:53+08:00
- repo_root: `/Users/joe/Documents/skill`

## Current Execution State

- task: Validate real Claude CLI host integration and audit shared Codex CLI/Desktop entrypoints
- phase: validated
- status: completed
- route: execution-controller-coding, skill-developer-codex
- next_actions: Fix Claude project subagent registration so .claude/agents becomes live instead of decorative. / Optionally run one interactive Codex Desktop GUI smoke test if you want host-level sign-off beyond repo-local entrypoint audit. / Keep Claude hooks, memory bridge, and MCP config under the shared generator and sync lane.
- scope: .supervisor_state.json / SESSION_SUMMARY.md / NEXT_ACTIONS.json / EVIDENCE_INDEX.json / TRACE_METADATA.json / codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py / tests/test_codex_agno_runtime_services.py / tests/test_codex_agno_runtime_runtime.py
- acceptance: Runtime recovery artifact enumeration includes .supervisor_state.json when the file exists at repo root. / Targeted runtime/checkpointer tests prove TRACE_RESUME_MANIFEST artifact_paths carry the supervisor-state path. / Continuity artifacts point to the same validated task, owner/gate story, and scope guardrails. / The slice remains explicitly inside thin projection + Rust contract-first migration and does not introduce runtime rewrite semantics.

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
- Hermes 只读取共享闭环产物，不再依赖旧宿主目录或注入链路
- 当前任务真实状态始终以 task artifacts 与 `.supervisor_state.json` 为准
- 复杂任务默认先走 `execution-controller-coding`

## Recent Lessons

- **L-1**: transport TRACE 日志会迅速膨胀，不能默认长期保留
- **L-2**: 可重建缓存不应混入长期记忆层
- **L-3**: 记忆 consolidation 要优先提炼稳定结论，而不是复制原始上下文

## Artifact Anchors

- `artifacts/current/SESSION_SUMMARY.md`
- `artifacts/current/NEXT_ACTIONS.json`
- `artifacts/current/EVIDENCE_INDEX.json`
- `artifacts/current/TRACE_METADATA.json`
- `.supervisor_state.json`
- `./.codex/memory/`
