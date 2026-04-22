# Skill Routing Index

> Entry point for rapid lookup.
> Prefer `skills/SKILL_ROUTING_RUNTIME.json` for the lean machine-readable route map.
> Prefer `skills/SKILL_MANIFEST.json` for the full manifest (includes owner, priority, source, etc.).
> RUNTIME (v2) is a compact 8-key subset: slug, layer, owner, gate, session_start, summary, trigger_hints, health.
> MANIFEST is the full 11-key record: slug, layer, owner, gate, priority, description, session_start, trigger_hints, health, source, source_position.

## 6-rule gate checklist
1. Extract object / action / constraints / deliverable first.
2. Check source gates before owners when the task starts from external evidence or official docs.
3. Check artifact gates when the primary object is a PDF, DOCX, XLSX, or similar file artifact.
4. Check evidence gates when screenshots, rendered pages, browser interaction, or root-cause debugging are central.
5. Check delegation gate before owner selection when the task is complex and parallel sidecars would help.
6. Only then choose the narrowest owner and add at most one overlay.

## Gates & Meta
| Name | Layer | Owner | Gate | Description |
|---|---|---|---|---|
| `execution-controller-app` | L0 | @app-controller | delegation | Master orchestrator for production-grade app optimization, refactor, and full-st |
| `execution-controller-coding` | L0 | @kernel-controller | delegation | Orchestrate complex execution with aggressive routing, state, delegation, and co |
| `gh-address-comments` | L0 | gate | source | Triage and address GitHub PR review comments and review threads for the current  |
| `gh-fix-ci` | L0 | gate | source | Triage failing GitHub Actions PR checks with `gh` and `scripts/inspect_pr_checks |
| `sentry` | L0 | gate | source | Inspect Sentry issues, events, releases, environments, and recent production exc |
| `subagent-delegation` | L0 | gate | delegation | Decide whether to split a complex task across sidecars or preserve the same stru |
| `systematic-debugging` | L0 | gate | evidence | Investigate unknown failures before fixing |
| `openai-docs` | L1 | gate | source | Use OpenAI docs MCP tools for current OpenAI API, model, Apps SDK, Codex, and do |
| `doc` | L3 | gate | artifact | Read, create, edit, repair, and review `.docx` Word documents when layout and Wo |
| `pdf` | L3 | gate | artifact | Read, create, edit, repair, and review PDFs when rendering and page layout matte |
| `playwright` | L3 | gate | evidence | Use a real browser when live evidence or page interaction is required |
| `slides` | L3 | gate | artifact | Route presentation-artifact work before choosing a slide-authoring lane. Use thi |
| `spreadsheets` | L3 | gate | artifact | Route workbook-native spreadsheet artifact work before choosing a narrower imple |
| `visual-review` | L3 | gate | evidence | Review screenshots, rendered pages, charts, and UI artifacts with image-grounded |
| `idea-to-plan` | L-1 | @strategic-orchestrator | delegation | Turn ambiguous ideas into evidence-backed plans with branch routing and compress |
| `skill-developer-codex` | L0 | owner | none | Design and tune Codex skill routing/framework behavior |
| `skill-writer` | L0 | owner | none | Shape one skill's wording, boundary, and token budget |
| `anti-laziness` | L1 | overlay | none | Fused overlay to detect/counter cognitive laziness and force empirical evidence. |
| `citation-management` | L1 | owner | none | Verify, normalize, de-duplicate, complete, and format academic citations and ref |
| `plan-to-code` | L2 | owner | none | Implement a concrete plan or spec into integrated code |
| `brainstorm-research` | L3 | owner | none | Expand early research ideas into multiple comparable directions and preserve the |
| `autoresearch` | L4 | owner | none | Orchestrate autonomous research through a recoverable loop of hypothesis, experi |
| `latex-compile-acceleration` | L4 | owner | none | Speed up LaTeX compile and preview workflows |
| `iterative-optimizer` | L0 | overlay | none | N-round optimization loops with built-in laziness immunity |
| `skill-routing-repair-codex` | L0 | owner | none | Patch routing misses with the smallest safe skill fix |
| `writing-skills` | L0 | overlay | none | Standardize and strengthen multiple `SKILL.md` files and shared skill-writing do |
| `api-integration-debugging` | L1 | owner | none | Diagnose and fix API integration failures at service boundaries. Produces reprod |
| `backend-runtime-debugging` | L1 | owner | none | Diagnose backend runtime failures: crashes, tracebacks, OOM, deadlocks, hanging  |
| `checklist-writting` | L1 | owner | none | Write a versioned execution-ready checklist once the strategy is fixed. |
| `coding-standards` | L1 | overlay | none | Enforce cross-stack coding standards: naming, readability, error handling, immut |
| `documentation-engineering` | L1 | owner | none | Write, review, and maintain project documentation such as README, API docs, ADRs |
| `error-handling-patterns` | L1 | overlay | none | Design cross-language error-handling architectures such as custom errors, retry/ |
| `execution-audit-codex` | L1 | overlay | none | Audit execution quality with evidence, sidecar-first collection, and compressed  |
| `frontend-debugging` | L1 | owner | none | Diagnose frontend runtime bugs with a five-layer model (component → state → rend |
| `imagegen` | L1 | owner | none | Generate or edit raster images through VibeProxy Local /v1/responses using the b |
| `information-retrieval` | L1 | owner | none | Run multi-round research before acting or recommending |
| `plugin-creator` | L1 | owner | none | Create a local Codex plugin scaffold with `.codex-plugin/plugin.json` and option |
| `prompt-engineer` | L1 | owner | none | Transform vague instructions into structured prompts with explicit role, constra |
| `refactoring` | L1 | owner | none | Plan and execute systematic code refactoring without changing behavior. Use when |
| `skill-creator` | L1 | owner | none | Create or update a Codex skill package with clear routing metadata, scope, and s |
| `skill-installer` | L1 | owner | none | Install Codex skills from curated sources or GitHub into `$CODEX_HOME/skills`. |
| `skill-maintenance-codex` | L1 | overlay | none | Maintain Codex skill-library operational health through validation, sync checks, |
| `skill-scout` | L1 | owner | none | Research external skill ecosystems and produce gap-analysis proposals for the lo |
| `tdd-workflow` | L1 | overlay | none | Run a Test-Driven Development workflow centered on the RED-GREEN-REFACTOR loop w |
| `test-engineering` | L1 | owner | none | Choose the right test layer, write maintainable tests, and stabilize flaky behav |
| `architect-review` | L2 | owner | none | Review software architecture, system design, and major structural code changes w |
| `build-tooling` | L2 | owner | none | Debug and design JS/TS/Python build tooling across package managers, lockfiles,  |
| `checklist-fixer` | L2 | owner | none | Execute fix lists and implementation plans with mandatory per-item verification  |
| `checklist-normalizer` | L2 | owner | none | Rewrite a messy checklist into an execution-ready form with explicit serial/para |
| `code-review` | L2 | overlay | none | Review code with structured findings and optional quality scoring. Use when the  |
| `css-pro` | L2 | owner | none | Architect maintainable CSS layout, responsive, animation, and vibrant design-tok |
| `data-wrangling` | L2 | owner | none | Clean, transform, validate, and pipeline structured or semi-structured data acro |
| `datastore-cache-queue` | L2 | owner | none | Diagnose and fix correctness issues across stores, caches, queues, and ORM-backe |
| `dependency-migration` | L2 | owner | none | Manage, audit, upgrade, and migrate project dependencies across npm, pip, Cargo, |
| `env-config-management` | L2 | owner | none | Design, audit, debug, and implement app configuration across env vars, `.env`, s |
| `gh-pr-triage` | L2 | owner | none | Triage GitHub pull requests by collecting PR metadata, comments, reviewer state, |
| `git-workflow` | L2 | owner | none | Safely execute Git operations and remote sync |
| `github-investigator` | L2 | owner | none | Deep GitHub repo research with issue/PR timeline and code-history evidence |
| `gitx` | L2 | owner | none | Run the safe Git review-fix-tidy-commit-merge-push workflow end to end. |
| `observability` | L2 | owner | none | Make production systems observable through logs, metrics, traces, dashboards, an |
| `shell-cli` | L2 | owner | none | Produce safe, portable shell commands, pipelines, and scripts that handle quotin |
| `slides-source-first` | L2 | owner | none | Build or revise slide workflows where source-of-truth authoring and artifact con |
| `sustech-mailer` | L2 | owner | none | Send emails from the SUSTech student mailbox via SMTP with auto-generated conten |
| `web-platform-basics` | L2 | owner | none | Explain and fix browser-native behavior at the platform layer before reaching fo |
| `academic-search` | L3 | owner | none | Execute structured academic literature searches using Semantic Scholar, arXiv, G |
| `accessibility-auditor` | L3 | owner | none | Find and fix user-blocking accessibility issues with concrete WCAG 2.1/2.2-groun |
| `api-design` | L3 | owner | none | Design, review, and refactor API interfaces covering REST, GraphQL, gRPC, versio |
| `api-load-tester` | L3 | owner | none | Design and run API load, stress, soak, and spike tests with k6, wrk, or autocann |
| `cloudflare-deploy` | L3 | owner | none | Deploy, publish, migrate, and operate applications on Cloudflare using Workers,  |
| `code-acceleration` | L3 | owner | none | Speed up code with measured rewrites, batching, caching, and parallel execution |
| `design-agent` | L3 | gate | none | Route named-product design references and brand-plus-motion source grounding bef |
| `docker` | L3 | owner | none | Produce minimal, secure Docker images with correct layer caching, multi-stage bu |
| `experiment-reproducibility` | L3 | owner | none | Ensure and manage research experiment reproducibility: environment capture, rand |
| `frontend-code-quality` | L3 | overlay | none | Enforce frontend code-quality rules such as ≤150-line files, early returns, and  |
| `frontend-design` | L3 | owner | none | Guide distinctive, high-end UI design: aesthetic direction, typography, color, m |
| `github-actions-authoring` | L3 | owner | none | Produce GitHub Actions workflow YAML with minimal permissions, stable cache keys |
| `graphviz-expert` | L3 | owner | none | Create Graphviz/DOT diagrams for precise, orthogonal, publication-quality flowch |
| `i18n-l10n` | L3 | overlay | none | Internationalization and localization overlay for web/mobile projects. Use for m |
| `infographic` | L3 | owner | none | Generate HTML/CSS/JS infographics — single-page long-form visuals, knowledge car |
| `jupyter-notebook` | L3 | owner | none | Create, scaffold, refactor, and normalize Jupyter notebooks (`.ipynb`) for exper |
| `linux-server-ops` | L3 | owner | none | Get services running and staying healthy on a Linux host — systemd units, revers |
| `mcp-builder` | L3 | owner | none | Design, build, review, and improve MCP servers and agent-facing tool interfaces. |
| `mermaid-expert` | L3 | owner | none | Create Mermaid diagrams for flowcharts, process diagrams, sequence diagrams, ERD |
| `monorepo-tooling` | L3 | owner | none | Design clean package boundaries and task orchestration for multi-package reposit |
| `motion-design` | L3 | owner | none | Design and implement high-end web animations, micro-interactions, and staggered  |
| `native-app-debugging` | L3 | owner | none | Debug desktop app issues across the Web-Native boundary |
| `npm-package-authoring` | L3 | owner | none | Build, refactor, and publish npm packages and JavaScript/TypeScript libraries in |
| `performance-expert` | L3 | owner | none | Audit and improve web performance with emphasis on Core Web Vitals, asset weight |
| `refresh` | L3 | owner | none | Build the next-turn execution prompt, copy it to the clipboard, and reply with o |
| `release-engineering` | L3 | owner | none | Build release pipelines from commit to published artifact. Use for versioning st |
| `screenshot` | L3 | owner | none | Capture desktop or system screenshots including full screen, a specific app wind |
| `security-threat-model` | L3 | owner | none | Repository-grounded threat modeling for applications, services, MCP servers, API |
| `skill-developer` | L3 | owner | none | Create, improve, debug, and audit Antigravity skills and `SKILL.md` files. Use w |
| `skill-installer-antigravity` | L3 | owner | none | Install Antigravity skills into the shared workspace skill library from local fo |
| `xlsx` | L3 | owner | none | Use after the `$spreadsheets` gate when the user explicitly wants an `openpyxl`  |
| `agent-memory` | L4 | owner | none | Design persistent agent memory across sessions |
| `agent-swarm-orchestration` | L4 | owner | none | Design and debug multi-agent systems with planners, routers, workers, reviewers, |
| `ai-research` | L4 | owner | none | AI/ML research engineering for model training, experiment pipelines, evaluation, |
| `algo-trading` | L4 | owner | none | Design, analyze, and implement algorithmic trading strategies, backtests, execut |
| `assignment-compliance` | L4 | owner | none | Check whether a homework or course-project submission satisfies the stated requi |
| `auth-implementation` | L4 | owner | none | Produce server-enforced auth flows with clean separation between authentication, |
| `chatgpt-apps` | L4 | owner | none | Build, scaffold, refactor, and troubleshoot ChatGPT Apps SDK applications that c |
| `chrome-extension-dev` | L4 | owner | none | Produce Chrome extensions for Manifest V3: Service Workers, minimal permissions, |
| `copywriting` | L4 | owner | none | Create persuasive commercial copy for landing pages, ads, product descriptions,  |
| `email-template` | L4 | owner | none | Produce cross-client HTML emails that render correctly in Outlook, Gmail, and Ap |
| `financial-data-fetching` | L4 | owner | none | Fetch, validate, normalize, and export real financial market data: OHLCV, financ |
| `go-pro` | L4 | owner | none | Deliver safe concurrent Go code with managed goroutine lifecycles, composable in |
| `humanizer` | L4 | owner | none | Naturalize existing prose into clearer, more human-sounding text. Use for: 精修, 文 |
| `javascript-pro` | L4 | owner | none | Deliver correct JavaScript code for ESM/CJS boundaries, browser vs Node runtime  |
| `literature-synthesis` | L4 | owner | none | Systematically screen, cluster, compare, and synthesize academic literature into |
| `mac-memory-management` | L4 | owner | none | Optimize Apple Silicon ML runtimes for memory pressure, throughput, and MPS stab |
| `math-derivation` | L4 | owner | none | Execute rigorous mathematical derivations and proofs |
| `nextjs` | L4 | owner | none | Deliver Next.js 14/15 applications with correct App Router, Server Component, an |
| `node-backend` | L4 | owner | none | Produce well-layered Node.js backend services with thin handlers, boundary valid |
| `paper-length-tuner` | L4 | owner | none | Diagnose paper length vs target page/word budget and produce a section-level exp |
| `paper-logic` | L4 | owner | none | Audit a paper's scientific defensibility under peer review: claims-vs- evidence  |
| `paper-notation-audit` | L4 | owner | none | Audit and enforce notation consistency across an academic paper: abbreviations,  |
| `paper-reviewer` | L4 | owner | none | Review a paper by abstract dimensions, not by sections. Default to the full `G0- |
| `paper-reviser` | L4 | owner | none | Execute the paper gate ledger one gate at a time. Default to sequential revision |
| `paper-visuals` | L4 | owner | none | Audit and improve paper figures, tables, captions, legends, axes, notes, and res |
| `paper-writing` | L4 | owner | none | Polish already-decided academic paper prose without changing evidence or claim b |
| `ppt-beamer` | L4 | owner | none | Create, revise, and compile presentation decks with LaTeX Beamer when you want e |
| `ppt-html-export` | L4 | owner | none | Use after the `$slides` gate when the user explicitly wants HTML slides plus a b |
| `ppt-markdown` | L4 | owner | none | Build slide decks from Markdown using Slidev or Marp. Use for explicit Markdown  |
| `ppt-pptx` | L4 | owner | none | Create source-first `deck.js` plus editable `.pptx` decks with PptxGenJS, theme- |
| `python-pro` | L4 | owner | none | Deliver production-grade Python 3.12+ code with clean async boundaries, strict t |
| `react` | L4 | owner | none | Deliver React 19+ components with correct hook dependencies, optimal Server Comp |
| `research-engineer` | L4 | owner | none | Provide rigorous technical critique, algorithm analysis, formal reasoning, compl |
| `rust-pro` | L4 | owner | none | Deliver ownership-correct Rust code that compiles without unnecessary clones, ma |
| `scientific-figure-plotting` | L4 | owner | none | Create, refactor, and review code-generated scientific figures for papers using  |
| `security-audit` | L4 | overlay | none | Audit implementation-level security defects in auth, injection, SSRF, CSRF, secr |
| `seo-web` | L4 | owner | none | Audit and optimize technical SEO for web apps: meta tags, structured data (JSON- |
| `sql-pro` | L4 | owner | none | Write, optimize, debug, and review SQL for PostgreSQL, MySQL, SQLite, and analyt |
| `statistical-analysis` | L4 | owner | none | Guide research statistics for test choice, effect sizes, uncertainty reporting,  |
| `svelte` | L4 | owner | none | Deliver Svelte 5 applications using runes-based reactivity ($state, $derived, $e |
| `tailwind-pro` | L4 | owner | none | Produce Tailwind CSS configurations with design tokens, plugin hooks, and framew |
| `typescript-pro` | L4 | owner | none | Deliver type-safe TypeScript 5.x+ code. Enforces strict mode, encodes domain con |
| `vercel-react-best-practices` | L4 | overlay | none | Apply Vercel-style React/Next.js best practices for App Router, Server Component |
| `vue` | L4 | owner | none | Deliver Vue 3 applications using Composition API with correct reactivity chains, |
| `web-scraping` | L4 | owner | none | Plan and implement web scraping and structured data extraction workflows. Use wh |
| `webhook-security` | L4 | owner | none | Secure webhook receivers and callback endpoints for Stripe, GitHub, Slack, Clerk |
| `youtube-summarizer` | L4 | owner | none | Extract transcripts from YouTube videos and turn them into summaries, notes, key |

See `skills/SKILL_ROUTING_LAYERS.md` for the full owner map and reroute rules.
