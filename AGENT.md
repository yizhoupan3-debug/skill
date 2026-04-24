# Shared Agent Policy

This repository is designed to be entered from `AGENTS.md` (Codex), `CLAUDE.md`
(Claude Code), or `GEMINI.md` (Gemini CLI). These files must project one shared
framework policy instead of forking per-host routing or memory rules.

## Default Behavior

- Reply in Chinese unless the user asks for another language.
- Keep answers direct, concise, and easy to scan.
- Execute safe read/search/test/build commands directly when the runtime allows.
- Default to a get-shit-done posture for clear local tasks: auto-continue safe,
  reversible work, keep ownership local, and verify before handoff.
- Do not silently choose an ambiguous interpretation when it would materially
  change the code, output, or risk surface; surface the assumption or ask.
- Prefer the smallest solution that fully solves the stated problem; do not add
  speculative abstractions, options, or future-proofing that was not requested.
- Keep edits surgical: touch only what the task requires, match local style,
  and clean up only the mess created by the current change.
- For non-trivial work, define success in a verifiable way before
  implementation and use that definition to drive execution.
- Ask before destructive actions, external publishing, or account-impacting work.

## Simplify First

- Prefer simplification before expansion: delete, merge, inline, or narrow an
  existing path before adding a new layer, branch, helper, or adapter.
- For GPT-family models, prefer the native Codex/OpenAI-compatible path over a
  Claude-host compatibility bridge unless the task is specifically testing
  Claude Code behavior.
- If two approaches both solve the task, prefer the one with fewer moving
  parts, fewer branching paths, and fewer files or surfaces to keep in sync.
- Prefer removing obsolete compatibility, transition, or fallback logic over
  wrapping it again, unless a real caller or rollout constraint still needs it.
- Keep hot paths simple: avoid repeated parse/serialize loops, repeated file
  I/O, copy-heavy data flow, and wrapper-on-wrapper structure when a direct path
  will do.
- Keep `AGENT.md` short and high-signal; if a rule only matters for one
  workflow, move it into the task-specific doc instead of bloating shared
  policy.

## Repo Landmarks

- `skills/` holds the shared routing and workflow bodies; read the selected
  `SKILL.md` before acting.
- `scripts/router-rs/` owns the Rust hook bridge, lifecycle commands, host-entrypoint
  projections, host-entrypoint sync, and generated-surface audits.
- `scripts/router-rs/` also owns framework profile compilation, shared contract
  normalization, workspace bootstrap defaults, host adapter projections, and
  framework contract artifact emission. Python may call and validate these
  Rust outputs, but must not maintain a second truth source, fallback emitter,
  bridge default table, or Python/Rust parity lane.
- Host entrypoint sync is Rust-owned:
  `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"`.
- `artifacts/current/<task_id>/`, `artifacts/current/task_registry.json`, and
  `.supervisor_state.json` are the durable task-state surfaces; do not treat
  chat text as the only recovery source.

## Communication Style

- Lead with the answer or result, not status reports, greetings, or self-talk.
- Use plain Chinese and everyday words by default.
- Explain things in plain language first; give internal terms only if they help.
- Avoid internal runtime, routing, framework, or tool jargon unless the user
  explicitly asks for it.
- If a technical term is necessary, explain it in simple words the first time.
- Keep the default reply to one short paragraph; use lists only when the content
  is genuinely list-shaped.
- Do not force personality, performative style, jokes, or deliberate roughness
  by default; sounding natural matters more than sounding theatrical.
- Keep the tone calm, friendly, and practical.

## Output Compaction

- For high-output local commands where exact raw output is not required, follow
  `RTK.md` and prefer the corresponding `rtk ...` wrapper.
- Treat `RTK.md` as repo-local operator guidance only; shared routing and
  policy truth still lives in this file plus the generated routing artifacts.

## Verification Defaults

- Verify the narrowest meaningful slice before handoff.
- For shared policy, host entrypoint, routing, or hook changes, prefer this
  order unless the task says otherwise: Rust-owned sync through
  `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- ...`,
  then targeted `cargo test --manifest-path ./scripts/router-rs/Cargo.toml`,
  plus the narrow JS/TS test only when browser-mcp code changed.
- If you skip a verification step that would normally matter, say so plainly in
  the closeout.

## Task Closeout

- Keep end-of-task user-facing closeouts in plain Chinese by default.
- Default the closeout to one short paragraph that covers exactly three points:
  what was done, what effect was achieved, and what still needs to happen next
  or that the work is finished.
- Keep the wording plain and natural; do not make the default closeout sound
  like a task artifact, audit log, or status machine.
- Prefer user-visible effect over implementation narration in the default
  closeout.
- If no further work is needed, say that directly instead of inventing follow-up
  tasks.
- Do not default to changed-file inventories, evidence lists, path dumps,
  changelog-style recaps, or step-by-step implementation retellings in the
  final user-facing closeout.
- Machine continuity artifacts such as `NEXT_ACTIONS.json`,
  `.supervisor_state.json`, and verification or blocker fields remain the
  recovery truth; do not mirror them verbatim into the user-facing closeout
  unless they materially affect the user's next decision.

## Policy Placement

- Put durable response policy in this file: answer-first phrasing, plain-language
  explanation, tone, closeout shape, and routing posture.
- Put deterministic runtime safeguards and narrow execution-time coding nudges
  in hooks: generated-surface protection, lifecycle refresh, environment
  reloads, failure alerts, and cheap repo-specific implementation reminders.
- Keep user-specific notifications, personal approvals, and machine-local
  preferences in `~/.claude/settings.json` or `.claude/settings.local.json`,
  not in committed project hooks.
- Do not use hooks to inject personality, carry general writing policy, or
  rewrite broad prompts when `AGENT.md` or `CLAUDE.md` can express the rule
  directly.
- Keep this file compact and factual. If a rule turns into a long workflow,
  move the procedure into `skills/`, `code_review.md`, or another task-specific
  doc and reference it instead of bloating this file.
- Add or tighten durable rules only after repeated real mistakes or verified
  friction.

## Turn-Start Routing

1. Extract `object / action / constraints / deliverable`.
2. Surface any ambiguity that would materially change the route or result.
3. Check gates before owners.
4. Use the narrowest matching skill and read its `SKILL.md` before acting.
5. For non-trivial execution, state the minimum success criteria and intended
   verification path before coding.
6. If no skill matches, consult `skills/SKILL_ROUTING_RUNTIME.json` first, then
   `skills/SKILL_ROUTING_INDEX.md`.
7. Keep one primary owner and at most one overlay.
8. Use `execution-controller-coding` for high-load or long-running work, and
   check `subagent-delegation` before splitting bounded sidecars.
9. Treat explicit `gsd` / `get shit done` / “推进到底” requests as a posture
   boost for `execution-controller-coding` plus `anti-laziness`, not as an
   external workflow.

## Shared Runtime Contract

- Runtime truth lives in `skills/`, task artifacts, and
  `.supervisor_state.json`.
- Host-specific entry files are thin projections only; they must not fork
  routing, memory schema, or artifact rules.
- Framework profile, shared contract, host adapter, bootstrap, bridge contract,
  compatibility inventory, and contract artifacts are Rust-owned surfaces.
  Do not reintroduce Python helper/bridge/compatibility code to produce them.
- Complex tasks externalize state into `SESSION_SUMMARY.md`,
  `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`, `TRACE_METADATA.json`,
  `.supervisor_state.json`, and `artifacts/current/task_registry.json`.
- `artifacts/current/<task_id>/` is task-local continuity and the primary task
  truth. Keep bootstrap, ops, evidence, and scratch outputs in their own roots.
- Root-level mirrors plus `artifacts/current/active_task.json` /
  `artifacts/current/focus_task.json` are focus-task projections only, not a
  parallel write surface.
- Shared continuity files are a single-writer surface: only the active
  integrator writes the shared focus projection; parallel lanes emit local deltas.

## Memory Contract

- Framework memory lives at `./.codex/memory/`; in this repo it resolves to
  `./memory/`, and both paths are one shared root.
- Default recall reads only the stable layer: `MEMORY.md`, `preferences.md`,
  `decisions.md`, `lessons.md`, `runbooks.md`, plus an active task summary only
  when clearly needed.
- Historical or debug snapshots belong under `memory/archive/` or
  `artifacts/ops/memory_automation/`, not the normal prompt path.
- Host entry files may reference framework memory, but must not redefine its
  schema or ownership.

## Workspace Binding

- When the user says `绑定xx目录` and the path is relative, resolve it under
  `/Users/joe/Documents`.
- Example: `绑定research/made` means `/Users/joe/Documents/research/made`.
- If the relative path does not exist there, ask for clarification instead of
  guessing across other roots.

## Runtime Sources Of Truth

- Default routing truth:
  `skills/SKILL_ROUTING_RUNTIME.json` and `skills/SKILL_ROUTING_INDEX.md`.
- Open the extended generated references only when you need ambiguity or audit
  detail: `skills/SKILL_ROUTING_LAYERS.md`,
  `skills/SKILL_SOURCE_MANIFEST.json`, `skills/SKILL_SHADOW_MAP.json`,
  `skills/SKILL_LOADOUTS.json`,
  `configs/framework/FRAMEWORK_SURFACE_POLICY.json`, and
  `skills/SKILL_APPROVAL_POLICY.json`.

## Host Entry Files

- Default host entrypoints: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`.
- Host-private overlays: `.claude/settings.json`, `.gemini/settings.json`.
- These files stay thin and point back to this shared policy.
- Claude entrypoint maintenance map:
  `docs/claude_entrypoint_maintenance.md`.
