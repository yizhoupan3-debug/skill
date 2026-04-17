# MEMORY

- workspace: skill
- generated_at: 2026-04-16T23:12:50+08:00
- source_root: /Users/joe/Documents/skill

## 稳定事实

- workspace: skill
- 当前主线任务：Validate real Claude CLI host integration and audit shared Codex CLI/Desktop entrypoints
- 当前阶段：validated
- 当前状态：completed
- 短期工件 contract 已启用，继续承担工作记忆职责。
- 长期记忆采用文件型、本地化、低依赖实现，不依赖外部服务。
- 复杂任务优先把状态外置到 SESSION_SUMMARY / NEXT_ACTIONS / EVIDENCE_INDEX / TRACE_METADATA / .supervisor_state。
- 高风险动作优先 report-first，禁止直接做破坏性清理。
- 已使用的路由/编排技能：execution-controller-coding, skill-developer-codex

## 当前任务态

- 当前主线任务：Validate real Claude CLI host integration and audit shared Codex CLI/Desktop entrypoints
- 当前阶段：validated
- 当前状态：completed
- 当前路由：execution-controller-coding, skill-developer-codex
- 下一步动作：Fix Claude project subagent registration so .claude/agents becomes live instead of decorative. / Optionally run one interactive Codex Desktop GUI smoke test if you want host-level sign-off beyond repo-local entrypoint audit. / Keep Claude hooks, memory bridge, and MCP config under the shared generator and sync lane.
- 阻塞项：暂无
- 作用域：.supervisor_state.json / SESSION_SUMMARY.md / NEXT_ACTIONS.json / EVIDENCE_INDEX.json / TRACE_METADATA.json / codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py / tests/test_codex_agno_runtime_services.py / tests/test_codex_agno_runtime_runtime.py
- 禁止范围：Rust runtime behavior changes / Python runtime behavior changes beyond artifact-path coverage / execution/delegation control-flow rewrites / execution_kernel.py branching changes / host entrypoint rewiring
- 验收标准：Runtime recovery artifact enumeration includes .supervisor_state.json when the file exists at repo root. / Targeted runtime/checkpointer tests prove TRACE_RESUME_MANIFEST artifact_paths carry the supervisor-state path. / Continuity artifacts point to the same validated task, owner/gate story, and scope guardrails. / The slice remains explicitly inside thin projection + Rust contract-first migration and does not introduce runtime rewrite semantics.
- 证据要求：Code diff in checkpoint_store.py showing supervisor state added to canonical runtime artifact enumeration. / Regression assertions in targeted runtime/checkpointer tests proving artifact_paths include .supervisor_state.json. / Verification command output for the targeted trace/runtime pytest suite. / Audit note confirming no remaining external consumer gap for the first-class control-plane artifacts.
- sidecar：external consumer audit for new control-plane contract artifacts / resumability/trace alignment audit for supervisor/control-plane descriptors
- 当前技能链：execution-controller-coding, skill-developer-codex

## 当前约束

- scope: .supervisor_state.json / SESSION_SUMMARY.md / NEXT_ACTIONS.json / EVIDENCE_INDEX.json / TRACE_METADATA.json / codex_agno_runtime/src/codex_agno_runtime/checkpoint_store.py / tests/test_codex_agno_runtime_services.py / tests/test_codex_agno_runtime_runtime.py
- forbidden_scope: Rust runtime behavior changes / Python runtime behavior changes beyond artifact-path coverage / execution/delegation control-flow rewrites / execution_kernel.py branching changes / host entrypoint rewiring
- acceptance_criteria: Runtime recovery artifact enumeration includes .supervisor_state.json when the file exists at repo root. / Targeted runtime/checkpointer tests prove TRACE_RESUME_MANIFEST artifact_paths carry the supervisor-state path. / Continuity artifacts point to the same validated task, owner/gate story, and scope guardrails. / The slice remains explicitly inside thin projection + Rust contract-first migration and does not introduce runtime rewrite semantics.
- evidence_required: Code diff in checkpoint_store.py showing supervisor state added to canonical runtime artifact enumeration. / Regression assertions in targeted runtime/checkpointer tests proving artifact_paths include .supervisor_state.json. / Verification command output for the targeted trace/runtime pytest suite. / Audit note confirming no remaining external consumer gap for the first-class control-plane artifacts.
- delegated_sidecars: external consumer audit for new control-plane contract artifacts / resumability/trace alignment audit for supervisor/control-plane descriptors
- open_blockers: 暂无

## 证据索引

- runtime: claude-auth-status (unknown)
- runtime: claude-stream-hook-session (unknown)
- runtime: claude-mcp-list (unknown)
- runtime: claude-debug-agent-not-found (unknown)
- runtime: codex-exec-startup-context (unknown)
- config: .claude/settings.json (unknown)
- config: .mcp.json (unknown)
- memory: memory/CLAUDE_MEMORY.md (unknown)
- doc: CLAUDE.md (unknown)
- doc: .claude/CLAUDE.md (unknown)
- doc: AGENTS.md (unknown)
- doc: .codex/model_instructions.md (unknown)
