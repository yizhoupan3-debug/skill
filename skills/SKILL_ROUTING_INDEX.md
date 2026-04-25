# Skill Routing Index

> Default read order: `skills/SKILL_ROUTING_RUNTIME.json` -> `skills/SKILL_ROUTING_INDEX.md`.
> Only open `skills/SKILL_MANIFEST.json` or `skills/SKILL_ROUTING_LAYERS.md` when the first two still leave owner/reroute ambiguity.

## Quick gate checklist
1. 规范: extract object / action / constraints / deliverable / success criteria first.
2. 计划: check source, artifact, evidence, and delegation gates before owner selection.
3. 计划: choose the narrowest domain owner and add at most one overlay.
4. 实施: take the smallest route delta and do not widen the abstraction.
5. 验证: close with tests, commands, screenshots, artifacts, or an explicit blocker.
6. Completion pressure changes route context only; it must not change selected owner.

## Gate shortcuts
| If the task starts with... | Route first | Why |
|---|---|---|
| OpenAI API / 模型 / 官方当前文档 | `openai-docs` | Use official OpenAI docs first for current OpenAI guidan |
| PR 评论 / review comment | `gh-address-comments` | Triage and address GitHub PR review comments and review  |
| CI 失败 / GitHub Actions 报红 | `gh-fix-ci` | Triage failing GitHub Actions PR checks with `gh` and th |
| Sentry 告警 / 线上异常 | `sentry` | Inspect Sentry issues, events, releases, environments, a |
| 根因未知的 bug / 失败 / 报错 | `systematic-debugging` | Investigate unknown failures before fixing |
| 需要并行 sidecar / 多代理拆分 | `subagent-delegation` | Decide whether a complex task should stay local, use bou |
| PDF / DOCX / 表格产物 | `pdf` | Read, create, edit, repair, and review PDFs when renderi |
| 浏览器实操取证 / 页面交互 | `playwright` | Use a real browser when live evidence or page interactio |
| 截图 / 页面 / 图表可视核查 | `visual-review` | Review screenshots, rendered pages, charts, and UI artif |

## Common lanes
| Common need | Route to | Why |
|---|---|---|
| 已有方案，直接落代码 | `plan-to-code` | Implement a concrete plan or spec into integrated code |
| 重构但不想改行为 | `refactoring` | Plan and execute systematic code refactoring without cha |
| 测试设计 / flaky / 补测试 | `test-engineering` | Choose the right test layer, write maintainable tests, a |
| 后端运行时问题 | `backend-runtime-debugging` | Diagnose backend runtime failures: crashes, tracebacks,  |
| 前端运行时问题 | `frontend-debugging` | Diagnose frontend runtime bugs with a five-layer model ( |
| README / ADR / 项目文档 | `documentation-engineering` | Write, review, and maintain project documentation such a |
| 构建 / 打包 / 工具链 | `build-tooling` | Debug and design JS/TS/Python build tooling across packa |
| Git 流程 / 合并 / 推送 | `gitx` | Run the safe Git review-fix-tidy-commit-merge-push workf |
| 多轮调研 / 对比 / 检索 | `information-retrieval` | Run multi-round research before acting or recommending |
| 科研项目 / 课题下一步 | `research-workbench` | Unified front door for non-manuscript research-project w |
| 文献梳理 / 搜论文 / novelty check | `literature-synthesis` | Screen, cluster, compare, and synthesize academic litera |
| skill 库 / 路由框架自身 | `skill-framework-developer` | Design and tune Codex skill routing/framework behavior |

## Optional overlays
| Add when... | Overlay | Why |
|---|---|---|
| 需要审查问题清单 | `code-review` | Review code with structured findings and optional qualit |
| 需要统一编码规范 | `coding-standards` | Enforce cross-stack coding standards: naming, readabilit |
| 需要多轮优化直到收敛 | `execution-audit` | Audit execution quality with evidence, sidecar-first col |

Need the full list? Use `skills/SKILL_ROUTING_RUNTIME.json` or `skills/SKILL_MANIFEST.json`.
Still ambiguous? See `skills/SKILL_ROUTING_LAYERS.md` for owner/reroute exceptions.
