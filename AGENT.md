# Shared Agent Policy

This repo is shared by Codex, Claude Code, and Gemini. Keep this file small:
only rules that should affect every host belong here.

## Defaults

- Reply in Chinese unless the user asks otherwise.
- Lead with the answer. Keep normal replies to one short paragraph.
- Use plain language first; explain jargon only when it helps.
- For clear local tasks, do the safe reversible work directly and verify the
  narrowest useful slice.
- Ask before destructive actions, external publishing, account-impacting work,
  or choices that would materially change risk or output.
- Prefer the smallest complete fix. Do not add speculative layers,
  compatibility paths, or future-proofing.
- Keep edits surgical: touch only what the task needs, follow local style, and
  clean up only the mess created by the current change.

## Routing

- Use a skill only when the request clearly matches one; read that skill's
  `SKILL.md` before acting.
- If routing is unclear, prefer the narrowest owner from
  `skills/SKILL_ROUTING_RUNTIME.json`, then `skills/SKILL_ROUTING_INDEX.md`.
- Keep one primary owner. Add an overlay only when it clearly reduces risk.

## Repo Rules

- `skills/` owns workflow bodies.
- `scripts/router-rs/` owns routing, hooks, host entrypoint sync, framework
  runtime surfaces, and generated-surface audits.
- Do not reintroduce Python bridge/helper parity for Rust-owned surfaces.
- Host entry files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`) stay thin and point
  back here.
- Use the repo sync script when editing host entrypoints.

## Continuity

- Use runtime state and artifacts as recovery truth; chat text is not enough.
- Keep default memory recall stable and short.

## Verification

- Verify the smallest meaningful slice before handoff.
- If a relevant verification step is skipped, say so plainly.
