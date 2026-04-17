# Skill Routing Root Index

> This is the lightweight entry point for tool routing. Read this first.

## Special Gates
| Skill | Layer | Gate | Description |
|---|---|---|---|
| `openai-docs` | — | source | Provide authoritative OpenAI API guidance from official developer docs. |
| `skill-evolution-guardian` | — | evidence | Gate skill to monitor and enforce skill library health and self-evolution. |
| `doc` | — | artifact | Read, create, edit, repair, and review `.docx` Word documents when layout and |
| `gh-address-comments` | L0 | source | Triage and address GitHub PR review comments and review threads for the |
| `gh-fix-ci` | L0 | source | Triage failing GitHub Actions PR checks with `gh` and |
| `pdf` | — | artifact | Read, create, edit, repair, and review PDFs when rendering and page layout |
| `playwright` | — | evidence | Automate a real browser from the terminal with Playwright CLI. |
| `sentry` | L0 | source | Inspect Sentry issues, events, releases, environments, and recent production exc |
| `subagent-delegation` | — | delegation | Decide whether a complex task should be split across Codex subagents, then |
| `systematic-debugging` | L0 | evidence | Diagnose bugs, failing tests, flaky behavior, build failures, and unexpected out |
| `visual-review` | — | evidence | Review screenshots, rendered pages, charts, and UI artifacts with |
| `xlsx` | — | artifact | Read, create, edit, repair, and review Excel `.xlsx` workbooks when formulas, |

## Meta & Process Skills (L0, L1)
| Skill | Layer | Gate | Description |
|---|---|---|---|
| `coding-standards` | L1 | none | Enforce cross-stack coding standards: naming, readability, error handling, |
| `frontend-debugging` | L1 | none | Diagnose frontend runtime issues through a structured five-layer model |
| `gh-address-comments` | L0 | source | Triage and address GitHub PR review comments and review threads for the |
| `gh-fix-ci` | L0 | source | Triage failing GitHub Actions PR checks with `gh` and |
| `iterative-optimizer` | L0 | none | N-round optimize→verify loops with built-in laziness immunity. |
| `plan-writing` | L1 | none | Write concise, structured execution plans with clear task breakdowns, |
| `prompt-engineer` | L1 | none | Transform vague or underspecified instructions into structured, controllable |
| `sentry` | L0 | source | Inspect Sentry issues, events, releases, environments, and recent production exc |
| `systematic-debugging` | L0 | evidence | Diagnose bugs, failing tests, flaky behavior, build failures, and unexpected out |
| `tdd-workflow` | L1 | none | Run a Test-Driven Development workflow centered on the RED-GREEN-REFACTOR |
| `test-engineering` | L1 | none | Choose the right test layer, write maintainable test code, and stabilize |
| `writing-skills` | L0 | none | Standardize and strengthen multiple `SKILL.md` files and shared skill-writing do |