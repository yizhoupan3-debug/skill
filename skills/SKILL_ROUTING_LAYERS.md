# Skill 分层路由详解

> 这是扩展参考，不是默认入口。
> 默认只看 `SKILL_ROUTING_RUNTIME.json`；不够再看 [SKILL_ROUTING_INDEX.md](file:///Users/joe/Documents/skill/skills/SKILL_ROUTING_INDEX.md)。
> 只有 owner / overlay / reroute 仍有歧义时，才打开本页。
> 协议细节见 [SKILL_FRAMEWORK_PROTOCOLS.md](file:///Users/joe/Documents/skill/skills/SKILL_FRAMEWORK_PROTOCOLS.md)
> 维护约定见 [SKILL_MAINTENANCE_GUIDE.md](file:///Users/joe/Documents/skill/skills/SKILL_MAINTENANCE_GUIDE.md)

适用场景：

- 你已经过了默认入口，但还在判断 owner / overlay / reroute 边界
- 你需要查某一层常见误用，或者处理技能重叠
- 你在做 skill 治理，而不是普通任务起步路由

## 路由顺序

每轮先提取 **对象 / 动作 / 约束 / 产物**，再按：

1. **先检查 gate**
2. **选择最窄 owner**
3. **最多叠 1 个 overlay**

## Automation lane

自动化是 **异步演化通道**，不参与同轮 owner 竞争：

- `skill-ci.yml`：push / PR 校验、生成物漂移拦截
- `evolution-audit.yml`：定时健康审计、同步 routing 产物、创建维护 issue
- Codex app automations（默认位于 `~/.codex/automations/<id>/automation.toml`；若显式设置 `CODEX_HOME`，则位于 `$CODEX_HOME/automations/<id>/automation.toml`）：
  异步收集维护任务、例行检查、产物刷新

自动化输出会在后续回合以 **evidence/source artifact** 形式进入路由，
再触发对应 gate 或 owner。

## Special Gates

| Gate | 先检查条件 | 角色 |
|---|---|---|
| `runtime delegation gate` | 复杂任务 + 可并行 sidecar + 仓库授权 | 运行时派单决策 |
| `systematic-debugging` | bug / 异常 / 失败 + 根因未知 | 先复现定位，再交回 owner |
| `openai-docs` | OpenAI API / 模型 / 产品 + 需官方当前文档 | source-of-truth gate |
| `design-md` | 用户需要持久设计 token、参考源、风格映射或验收合同，而不是直接改页面 | design source-grounding gate |
| `visual-review` | 已有截图 / 渲染图 / 可见证据 | evidence-first visual read |
| `pdf` / `doc` / `spreadsheets` | 主对象是 artifact 文件 | artifact-native workflow |
| `sentry` / `gh-address-comments` / `gh-fix-ci` | 任务由外部证据源触发 | source evidence gate |

## 分层概览

```text
L0  runtime execution controller, skill-framework-developer,
    gh-address-comments, gh-fix-ci, sentry, agent-swarm-orchestration,
    runtime delegation gate, systematic-debugging
L1  citation-management, deepinterview, documentation-engineering,
    image-generated, openai-docs
L2  gitx, paper-reviewer, paper-reviser, paper-workbench,
    paper-writing
L3  design-md, diagramming, doc, experiment-reproducibility,
    infographic, jupyter-notebook, pdf, refresh, screenshot,
    slides, spreadsheets, tao-ci, visual-review
L4  algo-trading, assignment-compliance,
    copywriting, email-template, financial-data-fetching,
    latex-compile-acceleration,
    mac-memory-management, math-derivation,
    ppt-beamer, ppt-pptx, source-slide-formats,
    scientific-figure-plotting,
    statistical-analysis, youtube-summarizer
Runtime lanes  planning, execution/code, language/framework, platform/integration,
               verification/review, memory and prompt policy, research workflow
```

> System skills（`.system/`）: `skill-creator`, `skill-installer`, `openai-docs`

## 各层何时做主 owner

| 层 | 做主 owner 的条件 | 不要误用 |
|---|---|---|
| **L0** | 任务本身是 skill 治理、路由、触发修复、框架自优化，或需要跨文件长周期的内核级指挥 (`runtime execution controller`) | 不要把普通实现问题抬到 L0 |
| **L1** | 执行方式是核心：计划、TDD、调试、重构、文档 | 根因已知时别默认 `systematic-debugging` |
| **L2** | 技术底座或运行时问题 | 语言/框架语义问题走更窄 skill |
| **L3** | 明确的平台、工具、产物、领域边界 | 不要把 L3 当泛化兜底 |
| **L4** | 高语义专业任务 | 不要用 L4 替代前置 gate |

## 易混淆边界

- `skill-framework-developer` vs `skill-creator` → 框架治理 / miss repair / wording modes vs 实际改一个 skill 包
- `skill-creator` vs `skill-installer` → 本地 authoring vs 新 skill intake / relink
- `systematic-debugging` vs 领域 owner → 根因未知 vs 根因已知
- `visual-review` vs `pdf` / `doc` / `spreadsheets` → 看证据 vs 改 artifact
- `spreadsheets` vs XLSX workflow → 通用 spreadsheet artifact gate owns `.xlsx`; workbook-native repair is a reference mode
- `slides` vs `ppt-pptx` → 通用 PPT / 现有 deck artifact gate vs 显式 `deck.plan.json` / Rust PPTX 源码工作流
- `slides` vs `source-slide-formats` → 通用演示文稿入口 vs 显式 Markdown / Slidev / Marp / HTML source slides
- `latex-compile-acceleration` vs `ppt-beamer` → 编译优化 vs Beamer 内容/版式
- research retrieval runtime vs `gh-address-comments` → repo / issue / PR / timeline 深挖 vs 当前 PR 状态汇总
- research retrieval runtime vs `skill-framework-developer` external scout mode → 通用调研 vs 为本地 skill 库做吸收式对标
- `runtime checklist planning` vs `runtime checklist execution` → 生成/整理 execution-ready checklist vs 按 checklist 执行
- `paper-workbench` vs `paper-reviewer` / `paper-reviser` / `paper-writing` → manuscript front door vs 明确只审 / 按 findings 改 / 局部文字

## 重路由信号

立即重路由，当且仅当：

- 用户显式改变目标
- 任务阶段自然迁移（plan → code → verify）
- 当前 skill 已连续 3 次落在 `## Do not use` 的边界外
- 证据源或产物类型发生变化
