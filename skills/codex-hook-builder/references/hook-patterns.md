# Useful Hook Patterns

Use these patterns as starting points. Install only the hooks that solve the user's concrete problem.

## Safety Gate

Goal: block irreversible local operations.

- Event: `PreToolUse`
- Matcher: shell-like tools and file-writing tools
- Blocks: `git reset --hard`, destructive `git clean`, `rm -rf` against root/home, `curl | sh`, `sudo`
- Template: `assets/templates/pre_tool_use_policy.py.tmpl`

## Approval Smoother

Goal: reduce repeated approval prompts without weakening safety.

- Event: `PermissionRequest`
- Allow only narrow, read-only commands such as `git status`, `git diff`, `rg`, `pwd`, and version checks.
- Deny obvious hazards.
- Return no decision for unknown commands so Codex falls back to normal user approval.
- Template: `assets/templates/permission_request.py.tmpl`

## Failure Feedback

Goal: make Codex respond to failed tools instead of burying the failure in a final answer.

- Event: `PostToolUse`
- Detect non-zero exit codes, tracebacks, failing tests, panics, and exception output.
- Block continuation with a short instruction when the failure needs attention.
- Template: `assets/templates/post_tool_use_review.py.tmpl`

## Secret Guard

Goal: avoid accidentally sending secrets into the agent loop.

- Event: `UserPromptSubmit`
- Detect common token/private-key patterns in prompt text.
- Block and ask the user to redact credentials.
- Template: `assets/templates/user_prompt_submit.py.tmpl`

## Project Guidance Loader

Goal: remind Codex to inspect local rules without injecting huge files.

- Event: `SessionStart`
- Detect root-level guidance files and add a one-sentence reminder.
- Do not paste full documents into hook output.
- Template: `assets/templates/session_start.py.tmpl`

## Finish Gate

Goal: prevent low-quality final answers after unverified work.

- Event: `Stop`
- Block only when the final response itself says validation was not run or was impossible.
- Avoid broad heuristics that cause loops.
- Template: `assets/templates/stop_gate.py.tmpl`

## Testing Pattern

Run a generated hook with fixture input:

```bash
python3 <skill>/scripts/run_hook_fixture.py.tmpl .codex/hooks/pre_tool_use_policy.py --event PreToolUse
```

Validate the hook file:

```bash
python3 <skill>/scripts/validate_hooks_json.py.tmpl .codex/hooks.json
```
