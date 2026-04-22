# Skill 分层路由详解

> 速查入口见 [SKILL_ROUTING_INDEX.md](file:///Users/joe/Documents/skill/skills/SKILL_ROUTING_INDEX.md)
> 协议细节见 [SKILL_FRAMEWORK_PROTOCOLS.md](file:///Users/joe/Documents/skill/skills/SKILL_FRAMEWORK_PROTOCOLS.md)
> 维护约定见 [SKILL_MAINTENANCE_GUIDE.md](file:///Users/joe/Documents/skill/skills/SKILL_MAINTENANCE_GUIDE.md)

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
| `subagent-delegation` | 复杂任务 + 可并行 sidecar + 仓库授权 | 运行时派单决策 |
| `systematic-debugging` | bug / 异常 / 失败 + 根因未知 | 先复现定位，再交回 owner |
| `openai-docs` | OpenAI API / 模型 / 产品 + 需官方当前文档 | source-of-truth gate |
| `design-agent` | 用户先要命名产品参考、品牌 token、参考源、风格映射，而不是直接改页面 | design source-grounding gate |
| `visual-review` | 已有截图 / 渲染图 / 可见证据 | evidence-first visual read |
| `pdf` / `doc` / `spreadsheets` | 主对象是 artifact 文件 | artifact-native workflow |
| `sentry` / `gh-address-comments` / `gh-fix-ci` | 任务由外部证据源触发 | source evidence gate |
| `playwright` | 需要 live browser 交互取证 | execution gate |

## 分层概览

```text
L0  execution-controller-coding, skill-writer, skill-developer-codex, skill-routing-repair-codex, writing-skills,
    gh-address-comments, gh-fix-ci, sentry, subagent-delegation,
    systematic-debugging, iterative-optimizer
L1  checklist-writting, tdd-workflow, test-engineering, refactoring,
    documentation-engineering, error-handling-patterns, frontend-debugging,
    backend-runtime-debugging,
    citation-management, coding-standards, prompt-engineer,
    information-retrieval, skill-scout, anti-laziness
L2  build-tooling, plan-to-code, api-integration-debugging,
    datastore-cache-queue, observability, web-platform-basics, git-workflow,
    css-pro, shell-cli, data-wrangling, dependency-migration,
    checklist-normalizer, checklist-fixer, env-config-management, code-review,
    architect-review, sustech-mailer, github-investigator
L3  academic-search, accessibility-auditor, api-design, api-load-tester,
    brainstorm-research, cloudflare-deploy, doc, docker,
    design-agent, experiment-reproducibility, frontend-code-quality, frontend-design,
    github-actions-authoring, graphviz-expert, i18n-l10n, imagegen,
    infographic, jupyter-notebook, linux-server-ops, mcp-builder,
    mermaid-expert, monorepo-tooling, native-app-debugging, npm-package-authoring, pdf,
    performance-expert, playwright, release-engineering, screenshot,
    security-threat-model, skill-developer, skill-installer-antigravity,
    spreadsheets, sustech-mailer, visual-review, xlsx
L4  nextjs, node-backend, auth-implementation, chatgpt-apps, react, vue, svelte,
    ppt-markdown, ppt-beamer, ppt-html-export, ppt-pptx,
    paper-logic, paper-notation-audit, paper-reviewer, paper-reviser,
    paper-visuals, paper-writing, paper-length-tuner, assignment-compliance,
    latex-compile-acceleration, security-audit, webhook-security,
    mac-memory-management, ai-research, autoresearch, algo-trading,
    financial-data-fetching, agent-memory, agent-swarm-orchestration,
    literature-synthesis, statistical-analysis, chrome-extension-dev,
    humanizer, scientific-figure-plotting, tailwind-pro,
    typescript-pro, python-pro, javascript-pro, rust-pro, go-pro, sql-pro,
    vercel-react-best-practices, seo-web, email-template, web-scraping,
    youtube-summarizer, copywriting, research-engineer, math-derivation
Overlays  coding-standards, tdd-workflow, error-handling-patterns, code-review,
          frontend-code-quality, writing-skills, iterative-optimizer,
          skill-routing-repair-codex, security-audit, i18n-l10n,
          vercel-react-best-practices, anti-laziness
```

> System skills（`.system/`）: `skill-creator`, `skill-installer`, `openai-docs`

## 各层何时做主 owner

| 层 | 做主 owner 的条件 | 不要误用 |
|---|---|---|
| **L0** | 任务本身是 skill 治理、路由、触发修复、框架自优化，或需要跨文件长周期的内核级指挥 (`execution-controller-coding`) | 不要把普通实现问题抬到 L0 |
| **L1** | 执行方式是核心：计划、TDD、调试、重构、文档 | 根因已知时别默认 `systematic-debugging` |
| **L2** | 技术底座或运行时问题 | 语言/框架语义问题走更窄 skill |
| **L3** | 明确的平台、工具、产物、领域边界 | 不要把 L3 当泛化兜底 |
| **L4** | 高语义专业任务 | 不要用 L4 替代前置 gate |

## 易混淆边界

- `skill-developer-codex` vs `skill-routing-repair-codex` → 框架 redesign vs 事后最小修补
- `skill-writer` vs `skill-creator` → 写法指导 vs 实际改 skill 包
- `skill-creator` vs `skill-installer` → 本地 authoring vs 新 skill intake / relink
- `systematic-debugging` vs 领域 owner → 根因未知 vs 根因已知
- `design-agent` vs `frontend-design` → 先定参考源 / verified tokens / borrow-adapt map vs 直接做视觉改版
- `design-agent` vs `motion-design` → 先拆品牌与动效来源 vs 直接做动效实现
- `visual-review` vs `pdf` / `doc` / `spreadsheets` → 看证据 vs 改 artifact
- `spreadsheets` vs `xlsx` → 通用 Excel / workbook artifact gate vs 显式 `openpyxl` / `pandas` / LibreOffice 兼容 lane
- `slides` vs `ppt-pptx` → 通用 PPT / 现有 deck artifact gate vs 显式 `deck.js` / PptxGenJS 源码工作流
- `slides` vs `ppt-html-export` → 通用演示文稿入口 vs 显式 HTML slides + browser-matched PDF
- `slides` vs `ppt-markdown` → 通用演示文稿入口 vs 显式 Slidev / Marp / Markdown source
- `build-tooling` vs `typescript-pro` / `python-pro` / `javascript-pro` → 构建链 vs 语言语义
- `latex-compile-acceleration` vs `ppt-beamer` → 编译优化 vs Beamer 内容/版式
- `information-retrieval` vs `skill-scout` → 通用调研 vs skill 生态专项对标
- `information-retrieval` vs `github-investigator` → 通用多源调研 vs repo / issue / PR / timeline 深挖
- `github-investigator` vs `skill-scout` → 仓库拆解复盘 vs 为本地 skill 库做吸收式对标
- `checklist-writting` vs `checklist-normalizer` → 从目标直接生 execution-ready checklist vs 整理已有 checklist / phase plan
- `checklist-normalizer` vs `checklist-fixer` → 重写 checklist 结构 vs 按 checklist 执行
- `plan-to-code` vs `checklist-normalizer` → spec/plan 直接落代码 vs 先把 checklist shape 稳定下来

## 重路由信号

立即重路由，当且仅当：

- 用户显式改变目标
- 任务阶段自然迁移（plan → code → verify）
- 当前 skill 已连续 3 次落在 `## Do not use` 的边界外
- 证据源或产物类型发生变化
