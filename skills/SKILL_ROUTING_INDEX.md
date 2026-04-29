# Skill Routing Index

> Default read order: `skills/SKILL_ROUTING_RUNTIME.json` -> `skills/SKILL_ROUTING_INDEX.md`.
> Only open `skills/SKILL_MANIFEST.json` or `skills/SKILL_ROUTING_LAYERS.md` when the first two still leave owner/reroute ambiguity.

## Quick gate checklist
1. 讨论: extract object / action / constraints / deliverable / success criteria first.
2. 规划: check source, artifact, and evidence gates before owner selection.
3. 规划: choose the narrowest domain owner and add at most one overlay.
4. 执行: take the smallest route delta and do not widen the abstraction.
5. 验证: close with tests, commands, screenshots, artifacts, or an explicit blocker.
6. Completion pressure changes route context only; it must not change selected owner.

## Gate shortcuts
| If the task starts with... | Route first | Why |
|---|---|---|
| OpenAI API / 模型 / 官方当前文档 | `openai-docs` | Use official OpenAI docs first for current OpenAI guidan |
| PR 评论 / review comment | `gh-address-comments` | Address GitHub PR review comments with gh-source-gate. |
| CI 失败 / GitHub Actions 报红 | `gh-fix-ci` | Triage and fix failing GitHub Actions PR checks with gh- |
| Sentry 告警 / 线上异常 | `sentry` | Inspect Sentry production errors and issue evidence read |
| PDF / DOCX / 表格产物 | `pdf` | Handle layout-aware PDF reading, editing, repair, and re |
| 截图 / 页面 / 图表可视核查 | `visual-review` | Review screenshots and rendered visual artifacts. |

## Common lanes
| Common need | Route to | Why |
|---|---|---|

## Optional overlays
| Add when... | Overlay | Why |
|---|---|---|

Need the full list? Use `skills/SKILL_ROUTING_RUNTIME.json` or `skills/SKILL_MANIFEST.json`.
Still ambiguous? See `skills/SKILL_ROUTING_LAYERS.md` for owner/reroute exceptions.
