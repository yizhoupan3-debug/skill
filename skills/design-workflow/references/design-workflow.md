# Design Workflow Reference

Use this compact loop:

1. Capture the design target, audience, and existing assets.
2. If the job is `DESIGN.md` creation/update/lint/diff/read/application, route to
   `$design-md` before continuing.
3. Decide whether the remaining job is `prompt generation` or `acceptance verdict`.
4. Keep reference tokens concrete: typography, color, spacing, surface, motion,
   interaction states, and anti-patterns.
5. For acceptance, compare actual output against the agreed tokens and return a
   verdict: pass, rework, or fail.
6. Route screenshot-based evidence to `visual-review` when rendering proof is
   needed.
