This file is the Codex runtime overlay. Repository policy lives in `../../AGENTS.md`.

Codex-only rules:
- Always respond in Chinese (中文). Keep technical terms such as React, API, and TypeScript in English.
- Default to brief answers that focus on results and conclusions. Do not proactively provide process details or reasoning unless the user asks for them.
- Prefer the lean runtime routing map `../../skills/SKILL_ROUTING_RUNTIME.json` before opening heavier routing docs.
- Keep code comments in English, concise and professional.
- Functions and methods must have doc comments that explain purpose, parameters, and return values.
- Mobile sticky-thread rule: if the user is on mobile / 手机端 and has not explicitly switched topics or asked for a new thread, treat follow-up messages as continuing the current task/thread by default. Do not implicitly reset task context.
- Mobile completion rule: on mobile / 手机端, stay silent while processing, then send exactly one concise user-facing reply immediately when the task is completed, partially completed with a clear next step, or blocked and requires user input.
- For long or complex tasks, prefer artifact-grounded continuity via `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, and `TRACE_METADATA.json` instead of replaying full chat history.
- Approval semantics should prefer generated policy artifacts (`../../skills/SKILL_APPROVAL_POLICY.json`) over prose inference when available.
- When modifying any `skills/**/SKILL.md`, follow `skills/SKILL_MAINTENANCE_GUIDE.md`, then run:
  - `python3 scripts/sync_skills.py --apply`
  - `python3 scripts/check_skills.py --verify-codex-link`
