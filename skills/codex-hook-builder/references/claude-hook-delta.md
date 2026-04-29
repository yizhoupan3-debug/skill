# Claude Hook Delta

Use this only when the user asks to port a Claude Code hook to Codex or asks what Codex can borrow from Claude.

## Borrow the Concepts

- Lifecycle hooks should create deterministic guardrails around agent behavior.
- Hooks should be testable with JSON fixtures.
- Hook outputs should be short, structured, and actionable.
- Permission automation should distinguish allow, deny, and no-decision fallback.
- Hooks should be packaged with skills/plugins when the behavior belongs to a reusable capability.

## Do Not Copy Blindly

- Do not assume Codex supports Claude's full event surface.
- Do not assume Codex supports HTTP, MCP, prompt, or agent hook handler types unless current OpenAI docs say so.
- Do not assume skill-scoped or agent-scoped hook declarations exist in Codex.
- Do not depend on Claude event names such as `PostToolUseFailure`, `PostToolBatch`, `PreCompact`, or `InstructionsLoaded` in Codex configs.

## Porting Strategy

1. Identify the Claude event and the user's real goal.
2. Map the goal to the closest Codex event in `codex-hook-events.md`.
3. Convert handler logic to a command hook script.
4. Replace Claude-only fields with Codex-supported output fields.
5. Run the script with `scripts/run_hook_fixture.py.tmpl`.
6. Validate `hooks.json` with `scripts/validate_hooks_json.py.tmpl`.
7. Tell the user which Claude behavior was preserved and which was approximated.
