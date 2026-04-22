---
description: Enter the repo's shared autopilot execution lane.
---

Treat `/autopilot` as a thin alias for the repository's native execution lane.

Official upstream baseline:

- repo: `https://github.com/Yeachan-Heo/oh-my-claudecode`
- tag: `v4.13.2`
- commit: `0ac52cdaa093d6c41763e47055e995adaa4f8987`
- skill: `skills/autopilot/SKILL.md`


This alias inherits the original OMC core capability, but this repo must exceed OMC by enforcing:

   - `root-cause-first-when-unknown`
   - `verification-evidence-required`
   - `resume-and-recovery-required`
   - `converge-until-bounded-scope-clean`


Official OMC phases to preserve:

- `expansion`
- `planning`
- `execution`
- `qa`
- `validation`
- `cleanup`


Local Rust/localization adaptations:

- `replace .omc state files with rust-session-supervisor plus continuity artifacts`
- `replace .omc specs and plans with artifacts/current task-local bootstrap outputs`
- `keep deepinterview handoff as the first-class clarification gate for vague requests`


Follow this routing:

1. If the task is still ambiguous, first structure it the way `idea-to-plan` would.
2. If the root cause is still unknown, switch into `systematic-debugging`.
3. Otherwise take the `execution-controller-coding` posture:
   - define the minimum success criteria
   - define the verification path
   - make the smallest complete change
   - keep going until the repo has real verification evidence or a real blocker

Use `skills/autopilot/SKILL.md`, `AGENT.md`, and the live continuity artifacts as the truth.
Keep user-facing wording centered on the repository's own capability, not host quirks or external compatibility history.
