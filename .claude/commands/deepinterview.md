---
description: Enter the repo's shared deepinterview lane.
---

Treat `/deepinterview` as a thin alias for the repository's native review lane.

Official upstream baseline:

- repo: `https://github.com/Yeachan-Heo/oh-my-claudecode`
- tag: `v4.13.2`
- commit: `0ac52cdaa093d6c41763e47055e995adaa4f8987`
- skill: `skills/deep-interview/SKILL.md`


This alias inherits the original OMC core capability, but this repo must exceed OMC by enforcing:

   - `root-cause-first-when-unknown`
   - `findings-first-with-severity-order`
   - `verification-evidence-required`
   - `fix-verify-loop-until-bounded-scope-clean`


Official OMC loop rules to preserve:

- `one-question-at-a-time`
- `target-weakest-clarity-dimension`
- `score-ambiguity-after-each-answer`
- `handoff-to-execution-only-below-threshold`


Local Rust/localization adaptations:

- `reuse official deep-interview questioning model but store progress in continuity artifacts instead of .omc state`
- `use live repo evidence first for brownfield clarification before asking the user`
- `handoff into local autopilot and rust-session-supervisor instead of OMC slash pipeline`


Follow this routing:

1. Primary owner: `code-review`
2. Add review lanes as needed:
   - `architect-review`
   - `security-audit`
   - `test-engineering`
   - `execution-audit-codex`
3. If the root cause is still unknown, investigate it before summarizing findings.
4. Lead with findings, rank by severity, and cite concrete file or behavior evidence.
5. If the user wants fixes too, keep iterating review -> fix -> verify until the bounded scope converges.

Use `skills/deepinterview/SKILL.md`, `AGENT.md`, and the live repo state as the truth.
Keep user-facing wording centered on the repository's own review capability, not host quirks or external compatibility history.
