# Skill Routing Index

> Entry point for rapid lookup.
> This file is a human-facing quick reference, not the runtime authority.
> Prefer `skills/SKILL_ROUTING_RUNTIME.json` for the lean machine-readable route map.
> Prefer `skills/SKILL_MANIFEST.json` for the fallback / enrichment manifest.
> Default view shows hot + warm skills only; cold long-tail skills stay in runtime/manifest for explicit routing.
> Visible here: 121 / 140 skills.

## 6-rule gate checklist
1. Extract object / action / constraints / deliverable first.
2. Check source gates before owners when the task starts from external evidence or official docs.
3. Check artifact gates when the primary object is a PDF, DOCX, XLSX, or similar file artifact.
4. Check evidence gates when screenshots, rendered pages, browser interaction, or root-cause debugging are central.
5. Check delegation gate before owner selection when the task is complex and parallel sidecars would help.
6. Only then choose the narrowest owner and add at most one overlay.

## Default routing surface
| Name | Layer | Owner | Gate | Exposure | Description |
|---|---|---|---|---|---|
| `idea-to-plan` | L-1 | @strategic-orchestrator | delegation | hot | Turn ambiguous ideas into evidence-backed plans with branch routing and compress |
| `execution-controller-app` | L0 | @app-controller | delegation | hot | Master orchestrator for production-grade app optimization, refactor, and full-st |
| `execution-controller-coding` | L0 | @kernel-controller | delegation | hot | Orchestrate complex execution with aggressive routing, state, delegation, and co |
| `subagent-delegation` | L0 | gate | delegation | hot | Decide whether to split a complex task across sidecars or preserve the same stru |
| `systematic-debugging` | L0 | gate | evidence | hot | Investigate unknown failures before fixing |
| `openai-docs` | L1 | gate | source | hot | Use OpenAI docs MCP tools for current OpenAI API, model, Apps SDK, Codex, and do |
| `doc` | L3 | gate | artifact | warm | Read, create, edit, repair, and review `.docx` Word documents when layout and Wo |
| `pdf` | L3 | gate | artifact | warm | Read, create, edit, repair, and review PDFs when rendering and page layout matte |
| `slides` | L3 | gate | artifact | warm | Create, edit, verify, and export editable `.pptx` slide decks. Use this artifact |
| `spreadsheets` | L3 | gate | artifact | warm | Create, edit, analyze, and review workbook-native spreadsheet artifacts. Use thi |
| `visual-review` | L3 | gate | evidence | warm | Review screenshots, rendered pages, charts, and UI artifacts with image-grounded |
| `gh-address-comments` | L0 | gate | source | warm | Triage and address GitHub PR review comments and review threads for the current  |
| `gh-fix-ci` | L0 | gate | source | warm | Triage failing GitHub Actions PR checks with `gh` and `scripts/inspect_pr_checks |
| `sentry` | L0 | gate | source | warm | Inspect Sentry issues, events, releases, environments, and recent production exc |
| `playwright` | L3 | gate | evidence | warm | Use a real browser when live evidence or page interaction is required |
| `xlsx` | L3 | gate | artifact | warm | Read, create, edit, repair, and review Excel `.xlsx` workbooks when spreadsheet- |
| `skill-developer-codex` | L0 | owner | none | hot | Design and tune Codex skill routing/framework behavior |
| `anti-laziness` | L1 | overlay | none | warm | Fused overlay to detect/counter cognitive laziness and force empirical evidence. |
| `plan-to-code` | L2 | owner | none | hot | Implement a concrete plan or spec into integrated code |
| `latex-compile-acceleration` | L4 | owner | none | warm | Speed up LaTeX compile and preview workflows |
| `citation-management` | L1 | owner | none | warm | Verify, normalize, de-duplicate, complete, and format academic citations and ref |
| `brainstorm-research` | L3 | owner | none | warm | Expand early research ideas into multiple comparable directions and preserve the |
| `autoresearch` | L4 | owner | none | warm | Orchestrate autonomous research through a recoverable loop of hypothesis, experi |
| `iterative-optimizer` | L0 | overlay | none | warm | N-round optimization loops with built-in laziness immunity |
| `execution-audit-codex` | L1 | overlay | none | warm | Audit execution quality with evidence, sidecar-first collection, and compressed  |
| `build-tooling` | L2 | owner | none | hot | Debug and design JS/TS/Python build tooling across package managers, lockfiles,  |
| `checklist-fixer` | L2 | owner | none | warm | Execute fix lists and implementation plans with mandatory per-item verification  |
| `git-workflow` | L2 | owner | none | hot | Safely execute Git operations and remote sync |
| `code-acceleration` | L3 | owner | none | warm | Speed up code with measured rewrites, batching, caching, and parallel execution |
| `nextjs` | L4 | owner | none | warm | Deliver Next.js 14/15 applications with correct App Router, Server Component, an |
| `python-pro` | L4 | owner | none | hot | Deliver production-grade Python 3.12+ code with clean async boundaries, strict t |
| `react` | L4 | owner | none | hot | Deliver React 19+ components with correct hook dependencies, optimal Server Comp |
| `typescript-pro` | L4 | owner | none | hot | Deliver type-safe TypeScript 5.x+ code. Enforces strict mode, encodes domain con |
| `api-integration-debugging` | L1 | owner | none | warm | Diagnose and fix API integration failures at service boundaries. Produces reprod |
| `backend-runtime-debugging` | L1 | owner | none | warm | Diagnose backend runtime failures: crashes, tracebacks, OOM, deadlocks, hanging  |
| `coding-standards` | L1 | overlay | none | warm | Enforce cross-stack coding standards: naming, readability, error handling, immut |
| `documentation-engineering` | L1 | owner | none | hot | Write, review, and maintain project documentation such as README, API docs, ADRs |
| `error-handling-patterns` | L1 | overlay | none | warm | Design cross-language error-handling architectures such as custom errors, retry/ |
| `frontend-debugging` | L1 | owner | none | warm | Diagnose frontend runtime bugs with a five-layer model (component → state → rend |
| `imagegen` | L1 | owner | none | warm | Generate or edit raster images through VibeProxy Local /v1/responses using the b |
| `information-retrieval` | L1 | owner | none | warm | Run multi-round research before acting or recommending |
| `plugin-creator` | L1 | owner | none | warm | Create a local Codex plugin scaffold with `.codex-plugin/plugin.json` and option |
| `prompt-engineer` | L1 | owner | none | warm | Transform vague instructions into structured prompts with explicit role, constra |
| `refactoring` | L1 | owner | none | hot | Plan and execute systematic code refactoring without changing behavior. Use when |
| `skill-creator` | L1 | owner | none | warm | Create or update a Codex skill package with clear routing metadata, scope, and s |
| `skill-installer` | L1 | owner | none | warm | Install Codex skills from curated sources or GitHub into `$CODEX_HOME/skills`. |
| `skill-maintenance-codex` | L1 | overlay | none | warm | Maintain Codex skill-library operational health through validation, sync checks, |
| `tdd-workflow` | L1 | overlay | none | warm | Run a Test-Driven Development workflow centered on the RED-GREEN-REFACTOR loop w |
| `test-engineering` | L1 | owner | none | hot | Choose the right test layer, write maintainable tests, and stabilize flaky behav |
| `architect-review` | L2 | owner | none | warm | Review software architecture, system design, and major structural code changes w |
| `code-review` | L2 | overlay | none | warm | Review code with structured findings and optional quality scoring. Use when the  |
| `css-pro` | L2 | owner | none | warm | Architect maintainable CSS layout, responsive, animation, and vibrant design-tok |
| `data-wrangling` | L2 | owner | none | warm | Clean, transform, validate, and pipeline structured or semi-structured data acro |
| `datastore-cache-queue` | L2 | owner | none | warm | Diagnose and fix correctness issues across stores, caches, queues, and ORM-backe |
| `dependency-migration` | L2 | owner | none | warm | Manage, audit, upgrade, and migrate project dependencies across npm, pip, Cargo, |
| `env-config-management` | L2 | owner | none | warm | Design, audit, debug, and implement app configuration across env vars, `.env`, s |
| `gh-pr-triage` | L2 | owner | none | warm | Triage GitHub pull requests by collecting PR metadata, comments, reviewer state, |
| `github-investigator` | L2 | owner | none | warm | Deep GitHub repo research with issue/PR timeline and code-history evidence |
| `observability` | L2 | owner | none | hot | Make production systems observable through logs, metrics, traces, dashboards, an |
| `shell-cli` | L2 | owner | none | warm | Produce safe, portable shell commands, pipelines, and scripts that handle quotin |
| `sustech-mailer` | L2 | owner | none | warm | Send emails from the SUSTech student mailbox via SMTP with auto-generated conten |
| `web-platform-basics` | L2 | owner | none | warm | Explain and fix browser-native behavior at the platform layer before reaching fo |
| `academic-search` | L3 | owner | none | warm | Execute structured academic literature searches using Semantic Scholar, arXiv, G |
| `accessibility-auditor` | L3 | owner | none | warm | Find and fix user-blocking accessibility issues with concrete WCAG 2.1/2.2-groun |
| `api-design` | L3 | owner | none | warm | Design, review, and refactor API interfaces covering REST, GraphQL, gRPC, versio |
| `api-load-tester` | L3 | owner | none | warm | Design and run API load, stress, soak, and spike tests with k6, wrk, or autocann |
| `cloudflare-deploy` | L3 | owner | none | warm | Deploy, publish, migrate, and operate applications on Cloudflare using Workers,  |
| `design-agent` | L3 | gate | none | warm | Route named-product design references and brand-plus-motion source grounding bef |
| `docker` | L3 | owner | none | warm | Produce minimal, secure Docker images with correct layer caching, multi-stage bu |
| `experiment-reproducibility` | L3 | owner | none | warm | Ensure and manage research experiment reproducibility: environment capture, rand |
| `frontend-code-quality` | L3 | overlay | none | warm | Enforce frontend code-quality rules such as ≤150-line files, early returns, and  |
| `frontend-design` | L3 | owner | none | warm | Guide distinctive, high-end UI design: aesthetic direction, typography, color, m |
| `github-actions-authoring` | L3 | owner | none | warm | Produce GitHub Actions workflow YAML with minimal permissions, stable cache keys |
| `graphviz-expert` | L3 | owner | none | warm | Create Graphviz/DOT diagrams for precise, orthogonal, publication-quality flowch |
| `i18n-l10n` | L3 | overlay | none | warm | Internationalization and localization overlay for web/mobile projects. Use for m |
| `infographic` | L3 | owner | none | warm | Generate HTML/CSS/JS infographics — single-page long-form visuals, knowledge car |
| `jupyter-notebook` | L3 | owner | none | warm | Create, scaffold, refactor, and normalize Jupyter notebooks (`.ipynb`) for exper |
| `linux-server-ops` | L3 | owner | none | warm | Get services running and staying healthy on a Linux host — systemd units, revers |
| `mcp-builder` | L3 | owner | none | warm | Design, build, review, and improve MCP servers and agent-facing tool interfaces. |
| `mermaid-expert` | L3 | owner | none | warm | Create Mermaid diagrams for flowcharts, process diagrams, sequence diagrams, ERD |
| `monorepo-tooling` | L3 | owner | none | warm | Design clean package boundaries and task orchestration for multi-package reposit |
| `motion-design` | L3 | owner | none | warm | Design and implement high-end web animations, micro-interactions, and staggered  |
| `npm-package-authoring` | L3 | owner | none | warm | Build, refactor, and publish npm packages and JavaScript/TypeScript libraries in |
| `performance-expert` | L3 | owner | none | warm | Audit and improve web performance with emphasis on Core Web Vitals, asset weight |
| `release-engineering` | L3 | owner | none | warm | Build release pipelines from commit to published artifact. Use for versioning st |
| `screenshot` | L3 | owner | none | warm | Capture desktop or system screenshots including full screen, a specific app wind |
| `security-threat-model` | L3 | owner | none | warm | Repository-grounded threat modeling for applications, services, MCP servers, API |
| `skill-installer-antigravity` | L3 | owner | none | warm | Install Antigravity skills into the shared workspace skill library from local fo |
| `agent-memory` | L4 | owner | none | warm | Design persistent agent memory across sessions |
| `agent-swarm-orchestration` | L4 | owner | none | warm | Design and debug multi-agent systems with planners, routers, workers, reviewers, |
| `ai-research` | L4 | owner | none | warm | AI/ML research engineering for model training, experiment pipelines, evaluation, |
| `algo-trading` | L4 | owner | none | warm | Design, analyze, and implement algorithmic trading strategies, backtests, execut |
| `assignment-compliance` | L4 | owner | none | warm | Check whether a homework or course-project submission satisfies the stated requi |
| `auth-implementation` | L4 | owner | none | warm | Produce server-enforced auth flows with clean separation between authentication, |
| `chatgpt-apps` | L4 | owner | none | warm | Build, scaffold, refactor, and troubleshoot ChatGPT Apps SDK applications that c |
| `chrome-extension-dev` | L4 | owner | none | warm | Produce Chrome extensions for Manifest V3: Service Workers, minimal permissions, |
| `copywriting` | L4 | owner | none | warm | Create persuasive commercial copy for landing pages, ads, product descriptions,  |
| `email-template` | L4 | owner | none | warm | Produce cross-client HTML emails that render correctly in Outlook, Gmail, and Ap |
| `financial-data-fetching` | L4 | owner | none | warm | Fetch, validate, normalize, and export real financial market data: OHLCV, financ |
| `go-pro` | L4 | owner | none | warm | Deliver safe concurrent Go code with managed goroutine lifecycles, composable in |
| `humanizer` | L4 | owner | none | warm | Naturalize existing prose into clearer, more human-sounding text. Use for: 精修, 文 |
| `javascript-pro` | L4 | owner | none | warm | Deliver correct JavaScript code for ESM/CJS boundaries, browser vs Node runtime  |
| `literature-synthesis` | L4 | owner | none | warm | Systematically screen, cluster, compare, and synthesize academic literature into |
| `mac-memory-management` | L4 | owner | none | warm | Optimize Apple Silicon ML runtimes for memory pressure, throughput, and MPS stab |
| `math-derivation` | L4 | owner | none | warm | Execute rigorous mathematical derivations and proofs |
| `node-backend` | L4 | owner | none | hot | Produce well-layered Node.js backend services with thin handlers, boundary valid |
| `research-engineer` | L4 | owner | none | warm | Provide rigorous technical critique, algorithm analysis, formal reasoning, compl |
| `rust-pro` | L4 | owner | none | hot | Deliver ownership-correct Rust code that compiles without unnecessary clones, ma |
| `scientific-figure-plotting` | L4 | owner | none | warm | Create, refactor, and review code-generated scientific figures for papers using  |
| `security-audit` | L4 | overlay | none | warm | Audit implementation-level security defects in auth, injection, SSRF, CSRF, secr |
| `seo-web` | L4 | owner | none | warm | Audit and optimize technical SEO for web apps: meta tags, structured data (JSON- |
| `sql-pro` | L4 | owner | none | warm | Write, optimize, debug, and review SQL for PostgreSQL, MySQL, SQLite, and analyt |
| `statistical-analysis` | L4 | owner | none | warm | Guide research statistics for test choice, effect sizes, uncertainty reporting,  |
| `svelte` | L4 | owner | none | warm | Deliver Svelte 5 applications using runes-based reactivity ($state, $derived, $e |
| `tailwind-pro` | L4 | owner | none | warm | Produce Tailwind CSS configurations with design tokens, plugin hooks, and framew |
| `vercel-react-best-practices` | L4 | overlay | none | warm | Apply Vercel-style React/Next.js best practices for App Router, Server Component |
| `vue` | L4 | owner | none | warm | Deliver Vue 3 applications using Composition API with correct reactivity chains, |
| `web-scraping` | L4 | owner | none | warm | Plan and implement web scraping and structured data extraction workflows. Use wh |
| `webhook-security` | L4 | owner | none | warm | Secure webhook receivers and callback endpoints for Stripe, GitHub, Slack, Clerk |
| `youtube-summarizer` | L4 | owner | none | warm | Extract transcripts from YouTube videos and turn them into summaries, notes, key |
| `native-app-debugging` | L3 | owner | none | warm | Debug desktop app issues across the Web-Native boundary |

Cold skills remain available in `skills/SKILL_ROUTING_RUNTIME.json` and `skills/SKILL_MANIFEST.json` when explicitly matched.
See `skills/SKILL_ROUTING_LAYERS.md` for the full owner map and reroute rules.
