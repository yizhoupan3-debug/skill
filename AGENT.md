# Shared Agent Policy

This repository is designed to be entered from `AGENTS.md` (Codex), `CLAUDE.md`
(Claude Code), or `GEMINI.md` (Gemini CLI). These files must project one shared
framework policy instead of forking per-host routing or memory rules.

## Default Behavior

- Reply in Chinese unless the user asks for another language.
- Keep answers direct and concise.
- Execute safe read/search/test/build commands directly when the runtime allows.
- Ask before destructive actions, external publishing, or account-impacting work.

## Turn-Start Routing

1. Extract `object / action / constraints / deliverable`.
2. Check gates before owners.
3. Use the narrowest matching skill.
4. Read the chosen `SKILL.md` before analysis, search, coding, or edits.
5. If no skill matches, consult `skills/SKILL_ROUTING_RUNTIME.json`, then
   `skills/SKILL_ROUTING_INDEX.md`.
6. Keep exactly one primary owner and at most one overlay.
7. For high-load, cross-file, or long-running tasks, invoke
   `execution-controller-coding` and maintain `.supervisor_state.json`.
8. For complex tasks, check `subagent-delegation` before deciding whether to
   split bounded sidecars.

## Shared Runtime Contract

- The shared runtime truth lives in `skills/`, task artifacts, and
  `.supervisor_state.json`.
- Host-specific entry files are thin projections only. They must not fork the
  routing truth, memory schema, or artifact contract.
- Complex tasks should externalize state into:
  `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`,
  `TRACE_METADATA.json`, and `.supervisor_state.json`.

## Memory Contract

- Long-term framework memory remains project-local at `./.codex/memory/`
  unless tooling explicitly switches roots.
- This path is shared framework state, not a Codex-only policy claim.
- Host entry files may reference framework memory, but must not redefine its
  schema or ownership.

## Workspace Binding

- When the user says `绑定xx目录` and the path is relative, resolve it under
  `/Users/joe/Documents`.
- Example: `绑定research/made` means `/Users/joe/Documents/research/made`.
- If the relative path does not exist there, ask for clarification instead of
  guessing across other roots.

## Runtime Sources Of Truth

- `skills/SKILL_ROUTING_RUNTIME.json`: machine-readable routing truth
- `skills/SKILL_ROUTING_INDEX.md`: human quick reference
- `skills/SKILL_ROUTING_LAYERS.md`: owner and reroute map
- `skills/SKILL_SOURCE_MANIFEST.json`: source precedence
- `skills/SKILL_SHADOW_MAP.json`: shadow audit
- `skills/SKILL_LOADOUTS.json`: loadout definitions
- `skills/SKILL_APPROVAL_POLICY.json`: approval policy registry

## Host Entry Files

- Codex: `AGENTS.md`, `.codex/model_instructions.md`
- Claude Code: `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/settings.json`
- Gemini CLI: `GEMINI.md`, `.gemini/settings.json`

These entry files must stay thin and point back to this shared policy.
