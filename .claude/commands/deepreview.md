---
description: Enter the repo's shared deepreview lane.
---

Treat `/deepreview` as a thin alias for the repository's native review lane.

Follow this routing:

1. Primary owner: `code-review`
2. Add review lanes as needed:
   - `architect-review`
   - `security-audit`
   - `test-engineering`
   - `execution-audit-codex`
3. Lead with findings, rank by severity, and cite concrete file or behavior evidence.
4. If the user wants fixes too, keep iterating review -> fix -> verify until the bounded scope converges.

Use `skills/deepreview/SKILL.md`, `AGENT.md`, and the live repo state as the truth.
Keep user-facing wording centered on the repository's own review capability, not host quirks or external compatibility history.
