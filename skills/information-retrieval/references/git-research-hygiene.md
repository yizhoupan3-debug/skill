# Git Research Hygiene

Use Git as a **research trace layer**, not as the owner of research routing.

## Good uses

- `git worktree` for parallel topic branches or side-by-side repo analysis
- `git notes` to attach evidence summaries without rewriting commit messages
- structured commit trailers for research outcomes, such as:
  - `Source:`
  - `Confidence:`
  - `Decision:`
  - `Follow-up:`

## When this helps

- long-running investigations
- multi-repo comparisons
- evidence that must survive across sessions
- keeping recommendation rationale near the artifact that used it

## Boundary rule

Do **not** route to `$gitx` just because the research process benefits from Git hygiene. Route there only when the user actually needs Git operations, repository-state repair, or branch strategy help.
