# Repository Research Template

Use this template when repository evidence, code history, and GitHub discussions are central.

## Round 1 — Repo baseline

Collect:
- repository summary
- default branch
- README
- top-level tree
- language mix
- key directories

Questions:
- What does the repository claim to do?
- What are the likely entrypoints?
- Which subdirectories are product-critical vs support code?

## Round 2 — Architecture pass

Inspect the highest-value paths only.

Capture:
- entrypoints
- main modules
- boundaries between subsystems
- tools, frameworks, and storage dependencies
- obvious extensibility hooks

Output:
- 3–7 architecture bullets
- one short reusable-pattern list

## Round 3 — Evolution pass

Inspect only the history relevant to the question.

Possible evidence:
- commits
- issues
- pull requests
- releases
- tags
- contributor concentration

Questions:
- What changed recently?
- What looks stable vs experimental?
- Which decisions are visible from merged PRs or issue threads?
- Are there abandoned branches of thought that should not be copied?

## Round 4 — Synthesis

Minimum structure:

```markdown
## GitHub Investigation Summary
- Target:
- Scope:
- Confidence:

### Executive Summary
### Architecture
### Timeline / Evolution
### Reusable Patterns
### Risks / Unknowns
### Sources
```

## Escalation notes

Reroute to `$information-retrieval` when the repo is only one source among many and repository history is not central.
