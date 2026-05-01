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
| PR 评论 / review comment | `gh-address-comments` | Address GitHub PR review comments and lightweight PR tri |
| CI 失败 / GitHub Actions 报红 | `gh-fix-ci` | Triage and fix failing GitHub Actions PR checks with gh- |
| Sentry 告警 / 线上异常 | `sentry` | Inspect Sentry production errors and issue evidence read |
| PDF 文件 | `pdf` | Handle layout-aware PDF reading, editing, repair, and re |
| DOCX / Word 文件 | `doc` | Handle layout-aware Word .docx creation, edits, and revi |
| Excel / CSV / 表格产物 | `spreadsheets` | Route workbook-native spreadsheet artifact work before c |
| 截图 / 页面 / 图表可视核查 | `visual-review` | Review screenshots and rendered visual artifacts. |

## Common lanes
| Common need | Route to | Why |
|---|---|---|
| 已有方案，直接落代码 | `autopilot` | Native repo autopilot workflow for end-to-end execution  |
| 需要先澄清或收敛判断 | `deepinterview` | Native repo deep-interview workflow for evidence-first c |
| 多 agent / 并行 lane / worker 边界 | `agent-swarm-orchestration` | Decide whether work should stay local, use bounded sidec |
| 截图 / 页面 / 图表可视核查 | `visual-review` | Review screenshots and rendered visual artifacts. |
| Git 流程 / 分支合并 / 推送 | `gitx` | Run the safe Git review-fix-tidy-commit-branch-merge-pus |
| PPT / slides / deck | `slides` | Route presentation, PPT, PPTX, and slide deck tasks. |
| PDF 文件 | `pdf` | Handle layout-aware PDF reading, editing, repair, and re |
| DOCX / Word 文件 | `doc` | Handle layout-aware Word .docx creation, edits, and revi |
| Excel / CSV / 表格产物 | `spreadsheets` | Route workbook-native spreadsheet artifact work before c |
| 设计规范 / DESIGN.md / token | `design-md` | Manage DESIGN.md design-system contracts and visual toke |
| OpenAI API / 模型 / 官方当前文档 | `openai-docs` | Use official OpenAI docs first for current OpenAI guidan |
| skill 库 / 路由框架自身 | `skill-framework-developer` | Design and tune Codex skill routing/framework behavior |

## Optional overlays
| Add when... | Overlay | Why |
|---|---|---|

Need the full list? Use `skills/SKILL_ROUTING_RUNTIME.json` or `skills/SKILL_MANIFEST.json`.
Still ambiguous? See `skills/SKILL_ROUTING_LAYERS.md` for owner/reroute exceptions.
