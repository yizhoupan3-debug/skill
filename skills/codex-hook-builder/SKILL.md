---
name: codex-hook-builder
description: Build, port, inspect, and validate OpenAI Codex hooks. Use when the user asks for Codex hooks, hook automation, approval policy, command safety gates, prompt guards, final-answer gates, hook debugging, hooks.json/config.toml setup, or migrating Claude Code hooks to Codex.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
source: project
trigger_hints:
  - Codex hooks
  - hook automation
  - hooks.json
  - config.toml hooks
  - approval policy
  - command safety gates
  - prompt guards
  - final-answer gates
  - migrate Claude hooks
---

# Codex Hook Builder

## Workflow

1. Inspect existing Codex hook state before changing files:

```bash
python3 <skill>/scripts/inspect_codex_hooks.py.tmpl --repo <project-root>
```

2. Map the user's goal to a supported Codex event. Read `references/codex-hook-events.md` when event choice or output shape matters.

3. Generate the smallest useful hook. Prefer project-local hooks for repo-specific policy and user-level hooks only for personal defaults.

4. Reuse bundled templates before writing new hook code:

- `assets/templates/session_start.py.tmpl` for project guidance reminders.
- `assets/templates/user_prompt_submit.py.tmpl` for secret and risky-prompt checks.
- `assets/templates/pre_tool_use_policy.py.tmpl` for dangerous command blocking.
- `assets/templates/permission_request.py.tmpl` for narrow approval automation.
- `assets/templates/post_tool_use_review.py.tmpl` for failed-tool feedback.
- `assets/templates/stop_gate.py.tmpl` for final-answer quality gates.

5. Validate `hooks.json` after every edit:

```bash
python3 <skill>/scripts/validate_hooks_json.py.tmpl <project-root>/.codex/hooks.json
```

6. Run at least one fixture test for each hook script:

```bash
python3 <skill>/scripts/run_hook_fixture.py.tmpl <project-root>/.codex/hooks/<hook>.py --event PreToolUse
```

7. Report exactly what was installed, how it was tested, and any Codex hook limitations that affect reliability.

## Scaffolding

Use the scaffold script for a fast project-local hook when the default template matches the request:

```bash
python3 <skill>/scripts/scaffold_hook.py.tmpl --repo <project-root> --event PreToolUse --name safety-gate --matcher 'Bash|shell' --enable-feature
```

The scaffold helper creates `.codex/hooks/<name>.py`, updates `.codex/hooks.json`, and can enable `[features] codex_hooks = true` in `.codex/config.toml`. Review the generated project hook before relying on it.

## Design Rules

- Prefer command hooks unless current Codex docs confirm another handler type.
- Keep hook output short, structured, and actionable.
- Block only when the hook has high confidence; otherwise add context or return no decision.
- Do not overwrite existing hook scripts or config without reading them first.
- Avoid long-running hooks. Default to 10 seconds unless the user explicitly needs more.
- Do not claim hooks are a complete security boundary. They are useful guardrails and workflow controls.

## Porting Claude Hooks

Read `references/claude-hook-delta.md` when converting Claude Code hooks to Codex. Preserve the user-visible behavior, but map it to Codex-supported events and command hooks instead of copying Claude-only event names or handler types.
