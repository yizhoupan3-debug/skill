---
name: ppt-markdown
description: |
  Build slide decks from Markdown using Slidev or Marp.
  Use for explicit Markdown slide workflows such as Slidev, Marp, or
  “用 Markdown 做个 slides”, especially when live preview and text-first
  authoring matter more than native `.pptx` editability. Let `$slides` absorb
  generic PPT intake first.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Slidev
  - Marp
  - Markdown slides
  - slides live preview
  - PPT markdown
  - 用 Markdown 做个 slides
  - Slidev presentation
  - Marp slides
  - fast Markdown-authored slides with live preview
  - export to HTML
runtime_requirements:
  commands:
    - npm
    - npx
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity]
  tags:
    - markdown
    - slides
    - slidev
    - marp
    - presentation
    - ppt
---

- **Dual-Dimension Audit (Pre: Slide-Structure/Logic, Post: Layout-Fidelity/PDF-Export Results)** → `$execution-audit-codex` [Overlay]

# PPT Markdown

Build slide decks as Markdown files first, preview them in the browser, then
export to HTML, PDF, or PPTX. Default to this skill when the user wants a
rapid, developer-friendly slide workflow where Markdown source is the single
source of truth.

## When to use

- Creating slide decks from Markdown with Slidev or Marp as the engine
- Building developer talks, tech shares, internal presentations, or lecture notes
- Turning `.md` notes, outlines, or README-style content into a polished deck
- When the user values Git-friendly, text-based presentation source
- When rapid iteration and live preview matter more than PowerPoint editability
- When the user explicitly names Slidev, Marp, or "Markdown slides"
- When the user explicitly chooses Markdown as the authoring surface after slide-format intake

## Do not use

- Do not use for generic PPT / presentation requests with no source-format decision yet; check `$slides` first
- Do not use when the user needs a native editable `.pptx`; use `$ppt-pptx`
- Do not use when the user wants full HTML/CSS layout control with browser-matched PDF export; use `$ppt-html-export`
- Do not use when the user wants LaTeX Beamer source plus compiled PDF; use `$ppt-beamer`
- Do not use when the user already has a Reveal.js project and wants bare HTML authoring; use `$ppt-html-export`
- Do not use when the primary deliverable must survive editing in PowerPoint by non-technical collaborators

## Engine Selection

Two engines are supported. Choose based on the user's needs:

| Criterion | Slidev | Marp |
|-----------|--------|------|
| Best for | interactive tech talks, live coding, Vue components | quick export, VS Code workflow, lightweight decks |
| Framework | Vue 3 + Vite | standalone CLI / VS Code extension |
| Live coding | ✅ Monaco Editor built-in | ❌ |
| Vue components | ✅ first-class | ❌ |
| Export formats | HTML, PDF, PNG, SPA | HTML, PDF, PPTX, PNG |
| PPTX export | ❌ (use PDF) | ✅ native |
| Themes | npm packages, UnoCSS | built-in (default, gaia, uncover) + custom CSS |
| Presenter mode | ✅ separate window + mobile | ✅ VS Code preview |
| Setup weight | heavier (Node project) | lighter (single CLI or VS Code) |

**Default engine**: Marp — for its zero-config simplicity and direct PPTX export.
**Prefer Slidev**: when the user needs live coding, Vue components, interactive elements, or a full SPA deployment.

## Workflow

1. Create or reuse a project folder:
   - `slides.md` (main presentation source)
   - `assets/` (images, diagrams)
   - `sources.md` (citation log)
2. Choose the engine:
   - **Marp**: install `@marp-team/marp-cli` via npx or globally.
   - **Slidev**: initialize with `npx slidev@latest` or add to an existing Node project.
3. Write the slide deck in Markdown:
   - Use `---` (triple dash) as slide separators.
   - Add frontmatter for metadata (title, theme, paginate).
   - Use Marp directives (`<!-- _class: lead -->`) or Slidev frontmatter per slide.
   - Follow the content rules below for text quality.
4. Collect images before final writing:
   - Use user-provided assets first.
   - Download suitable images to local `assets/`.
   - Use AI image generation as fallback.
   - Do not leave remote URLs in the final deck.
5. Preview and iterate:
   - **Marp**: `npx @marp-team/marp-cli --preview slides.md`
   - **Slidev**: `npx slidev slides.md`
   - Fix content, styling, and density issues in the Markdown source directly.
6. Export:
   - **Marp → PDF**: `npx @marp-team/marp-cli slides.md --pdf`
   - **Marp → PPTX**: `npx @marp-team/marp-cli slides.md --pptx`
   - **Marp → HTML**: `npx @marp-team/marp-cli slides.md`
   - **Slidev → PDF**: `npx slidev export slides.md`
   - **Slidev → SPA**: `npx slidev build slides.md`
7. QA the exported output:
   - Open the PDF/HTML and inspect every slide for overflow, clipping, missing images, and font issues.
   - Use `$visual-review` on screenshots for structured QA when polish matters.
8. Deliver the `.md` source, exported output, and `assets/` together.

## Non-Negotiables

- The single source of truth is always the `.md` file. Do not hand-edit exports.
- Use local relative paths for all images in the final deck.
- Use `---` slide separators consistently; do not mix separator styles.
- Default to 16:9 aspect ratio unless explicitly requested otherwise.
- Do not accept Chinese orphan lines (1–2 chars alone on a line). Rewrite to fix.
- Keep mixed-language tokens intact (English terms, percentages, citations).
- Keep titles and headings visually balanced; no tiny trailing lines.
- Cite every quantitative claim and every external image source.
- Never fabricate experimental results or data.
- Prefer fewer words and larger text over dense small-font slides.
- **Superior Quality Audit**: For high-impact presentations, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Practical Defaults

- Default output: Markdown source + PDF export (Marp) or HTML SPA (Slidev).
- Default engine: Marp (zero-config, PPTX-capable).
- Default theme: Marp `default` or Slidev `default`; override when the user wants a branded look.
- Default layout: 16:9, one clear message per slide.
- Default density: 3–5 bullets or 2–3 content zones per slide.
- Default image sourcing: user-provided → online search → AI-generated fallback.
- Default citation policy: keep `sources.md` alongside the deck.
- Default QA: preview → fix → export → visual spot-check → `$visual-review` for polish-critical decks.

## Resource Guide

- Run [scripts/setup_marp.sh](./scripts/setup_marp.sh) to bootstrap a Marp project.
- Run [scripts/setup_slidev.sh](./scripts/setup_slidev.sh) to bootstrap a Slidev project.
- Copy [assets/slides.template.md](./assets/slides.template.md) as the starting point for a new Marp deck.
- Copy [assets/slidev.template.md](./assets/slidev.template.md) as the starting point for a new Slidev deck.

## Final Checks

- Confirm the `.md` source is the canonical version; no hand-edited exports.
- Confirm slide count in export matches the number of `---` separators + 1.
- Confirm all images are local and render in the export.
- Confirm no tiny-text workarounds; body text readable without zooming.
- Confirm no Chinese orphan lines; titles balanced; mixed-language tokens intact.
- Confirm the export format matches the user's stated need (PDF / PPTX / HTML).
- "强制进行 PPT 深度审计 / 检查页面布局与导出结果一致性。"
- "Use $execution-audit-codex to audit this slide deck for layout-fidelity idealism."
