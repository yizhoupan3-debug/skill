# Codex Hook Events

Use this reference when mapping a request to Codex hooks. Keep generated hooks aligned with the current OpenAI Codex hooks docs: https://developers.openai.com/codex/hooks.

## Supported Core Events

| Event | Best use | Typical output |
| --- | --- | --- |
| `SessionStart` | Add concise project guidance when Codex starts or resumes. | `additionalContext` only; avoid blocking startup unless absolutely necessary. |
| `UserPromptSubmit` | Detect secrets, unsafe requests, missing required context, or route-specific reminders before Codex acts. | `decision: block` for unsafe prompt, or `additionalContext` for guidance. |
| `PreToolUse` | Deny dangerous commands or add context before supported tool calls. | `permissionDecision: deny` with a clear reason. |
| `PermissionRequest` | Auto-allow known low-risk operations or auto-deny obvious hazards. | `decision.behavior: allow` or `deny`. |
| `PostToolUse` | React to failed commands, test failures, lint output, or generated artifacts. | `decision: block` for failures that require agent attention; `additionalContext` for soft feedback. |
| `Stop` | Prevent premature final answers when validation or disclosure is missing. | `decision: block` with a concise instruction. |

## Event Selection

- Use `UserPromptSubmit` when the policy depends on the user's prompt text.
- Use `PreToolUse` when the policy depends on a proposed tool or shell command.
- Use `PermissionRequest` when the goal is approval automation, not general validation.
- Use `PostToolUse` when the policy depends on command output, generated files, or exit status.
- Use `Stop` when the policy is about whether the agent may finish.
- Use `SessionStart` when the policy should add stable context at the beginning of a session.

## Practical Boundaries

- Treat hooks as a control plane, not a perfect security sandbox.
- Prefer command hooks today; do not assume Claude-style HTTP, MCP, prompt, or agent hook handlers are available in Codex unless current docs say so.
- Validate hook JSON locally before installing it.
- Simulate every hook script with fixture input before relying on it.
- Keep hook decisions short and actionable. The model should know exactly what to do next.

## Matching Notes

- Keep matchers narrow when blocking or approving tools.
- Prefer explicit tool names such as `Bash`, `shell`, `apply_patch`, `Edit`, or `Write` only after checking what the current Codex environment emits.
- Use `*` only for hooks that are safe for all matching invocations.
- Remember that multiple matched command hooks may not provide a stable ordered control flow. Do not design one hook to depend on another hook's side effects unless verified in the target environment.
