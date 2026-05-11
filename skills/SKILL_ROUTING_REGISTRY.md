# Skill Routing Registry

| Skill | Status | P | Layer | Owner | Gate | Source | Description |
|---|---|---|---|---|---|---|---|
| `plugin-creator` | Active | P1 | L0 | owner | none | system | Scaffold a local Codex plugin and optional marketplace entry |
| `skill-creator` | Active | P1 | L0 | owner | none | system | Design or revise one Codex skill package |
| `skill-installer` | Active | P1 | L0 | owner | none | system | Install curated or remote Codex skills |
| `agent-swarm-orchestration` | Active | P1 | L0 | gate | delegation | project | Decide whether work should stay local, use bounded sidecars, or escalate to team |
| `algo-trading` | Active | P2 | L4 | owner | none | project | Design, analyze, and implement algorithmic trading strategies, backtests, execut |
| `assignment-compliance` | Active | P2 | L4 | owner | none | project | Check whether a homework or course-project submission satisfies the stated requi |
| `autopilot` | Active | P1 | L0 | owner | none | project | Repo-native `/autopilot`：goal-style 连续执行、地平线切片、continuity 硬接力； bounded sidecar 并 |
| `citation-management` | Active | P2 | L1 | owner | none | project | Verify and format academic citations and bibliographies. |
| `code-review-deep` | Active | P2 | L2 | owner | none | project | Deep adversarial-style code review (review-only verdict first). Use for whole-PR |
| `copywriting` | Active | P2 | L4 | owner | none | project | Create persuasive commercial copy for landing pages, ads, product descriptions,  |
| `deepinterview` | Active | P2 | L1 | owner | none | project | Repo-native deepinterview workflow for evidence-first clarification and converge |
| `design-md` | Active | P1 | L3 | gate | artifact | project | Manage DESIGN.md design-system contracts and visual tokens. |
| `diagramming` | Active | P2 | L3 | owner | none | project | Create Mermaid or Graphviz/DOT diagrams for flowcharts, process diagrams, sequen |
| `doc` | Active | P1 | L3 | gate | artifact | project | Handle layout-aware Word .docx creation, edits, and review. |
| `documentation-engineering` | Active | P2 | L1 | owner | none | project | Write, review, and maintain project documentation such as README, API docs, ADRs |
| `email-template` | Active | P2 | L4 | owner | none | project | Produce cross-client HTML emails that render correctly in Outlook, Gmail, and Ap |
| `experiment-reproducibility` | Active | P2 | L3 | owner | none | project | Ensure and manage research experiment reproducibility: environment capture, rand |
| `financial-data-fetching` | Active | P2 | L4 | owner | none | project | Fetch, validate, normalize, and export real financial market data: OHLCV, OHLCV, |
| `gh-address-comments` | Active | P2 | L0 | gate | source | project | Address GitHub PR review comments and lightweight PR triage summaries with gh-so |
| `gh-fix-ci` | Active | P2 | L0 | gate | source | project | Triage and fix failing GitHub Actions PR checks with gh-source-gate. |
| `gitx` | Active | P1 | L2 | owner | none | project | Run the Git closeout workflow with deep review on the substantive diff before co |
| `hatch-pet` | Active | P1 | L3 | owner | none | project | Create, repair, validate, preview, and package Codex-compatible animated pet spr |
| `image-generated` | Active | P2 | L1 | owner | none | project | Generate or edit raster images through official OpenAI API using the bundled Rus |
| `infographic` | Active | P2 | L3 | owner | none | project | Generate HTML/CSS/JS infographics — single-page long-form visuals, knowledge car |
| `jupyter-notebook` | Active | P2 | L3 | owner | none | project | Create, scaffold, refactor, and normalize Jupyter notebooks (`.ipynb`) for exper |
| `latex-compile-acceleration` | Active | P1 | L4 | owner | none | project | Speed up LaTeX compile and preview workflows |
| `loop` | Active | P1 | L1 | owner | none | project | Adversarial multi-pass review-fix-verify with progressive rubric disclosure (sup |
| `mac-memory-management` | Active | P2 | L4 | owner | none | project | Optimize Apple Silicon ML runtimes for memory pressure, throughput, and MPS stab |
| `math-derivation` | Active | P2 | L4 | owner | none | project | Execute rigorous mathematical derivations and proofs |
| `openai-docs` | Active | P1 | L1 | gate | source | project | Use official OpenAI docs first for current OpenAI guidance |
| `paper-reviewer` | Active | P2 | L2 | owner | none | project | Specialist review lane behind `$paper-workbench`. Use when the user clearly want |
| `paper-reviser` | Active | P2 | L2 | owner | none | project | Specialist revision lane behind `$paper-workbench`. Use when the route is alread |
| `paper-workbench` | Active | P2 | L2 | owner | none | project | Unified front door for paper work. Use when the user has a manuscript-level task |
| `paper-writing` | Active | P2 | L2 | owner | none | project | Write, restructure, or polish bounded academic-paper prose after the claim/evide |
| `pdf` | Active | P1 | L3 | gate | artifact | project | Handle layout-aware PDF reading, editing, repair, and review. |
| `plan-mode` | Active | P2 | L1 | owner | none | project | Cursor Plan / 策划文档闸门 owner：先调研与 review，再产出可验收 todo 与修订闭环；`plan_profile: executio |
| `ppt-beamer` | Active | P2 | L4 | owner | none | project | Create, revise, and compile presentation decks with LaTeX Beamer when you want e |
| `spreadsheets` | Active | P1 | L3 | gate | artifact | project | Route workbook-native spreadsheet artifact work before choosing a narrower imple |
| `scientific-figure-plotting` | Active | P2 | L4 | owner | none | project | Create, refactor, and review code-generated scientific figures for papers using  |
| `screenshot` | Active | P2 | L3 | owner | none | project | Capture desktop or system screenshots including full screen, a specific app wind |
| `sentry` | Active | P2 | L0 | gate | source | project | Inspect Sentry production errors and issue evidence read-only. |
| `skill-framework-developer` | Active | P1 | L0 | owner | none | project | Design and tune Codex skill routing/framework behavior |
| `slides` | Active | P1 | L3 | gate | artifact | project | Route presentation, PPT, PPTX, and slide deck tasks. |
| `source-slide-formats` | Active | P2 | L4 | owner | none | project | Build presentation sources in Markdown, Slidev, Marp, or HTML/CSS and export the |
| `statistical-analysis` | Active | P2 | L4 | owner | none | project | Guide research statistics for test choice, effect sizes, uncertainty reporting,  |
| `systematic-debugging` | Active | P1 | L0 | gate | evidence | project | Investigate unknown failures before fixing |
| `tao-ci` | Active | P2 | L3 | owner | none | project | Draft professor-specific summer research outreach emails from workbook data |
| `tikz-paper-figure` | Active | P1 | L3 | owner | none | project | Convert AI/raster drafts into paper-ready TikZ standalone figures |
| `update` | Active | P1 | L0 | owner | none | project | Rust maint entrypoints + full codegen + all contract tests + optional host publi |
| `visual-review` | Active | P1 | L3 | gate | evidence | project | Review screenshots and rendered visual artifacts. |
| `youtube-summarizer` | Active | P2 | L4 | owner | none | project | Extract transcripts from YouTube videos and turn them into summaries, notes, key |
