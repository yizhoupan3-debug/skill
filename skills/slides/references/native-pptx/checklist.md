# PPTX Deck Checklist

## Non-Negotiables

### Core Deliverable
- Deliver a real editable `.pptx`, not only PDF, screenshots, or browser export.
- Keep `deck.plan.json` as the source of truth for generated decks.
- Use local relative paths for assets in the authoring workflow.
- Set theme fonts explicitly; do not rely on PowerPoint defaults.
- Do not use alternate package wrappers, script templates, or external Office inspectors in this skill.

### Visual System
- Build around a declared visual system before adding dense content.
- When source materials exist, produce or reuse a `DESIGN.md` / visual contract
  before styling the deck.
- Use 2–3 reusable layout families, not one-off slide designs.
- Make cover and closing slides feel designed and related.
- No visual drift, AI-slop, or anti-pattern relapse after rendered review.
- Avoid stock-theme styling, accidental SmartArt aesthetics, default chart palettes.
- The cover must follow one approved family (`dark-premium` or
  `light-editorial`) and keep that family consistent with section and closing
  slides.
- Make images feel embedded (framed panels, caption bands, overlays) not pasted.
- No important text on busy images without a solid protection layer.
- On dark slides, readability is a hard constraint — no low-contrast gray-on-black.

### Text Quality
- Outline text was polished before layout: built-in Rust copy naturalization for
  ordinary prose, `$copywriting` for persuasive decks, or `$paper-writing` for
  academic decks.
- Generated titles, bullets, captions, and speaker notes use direct claims
  instead of meta narration.
- No filler phrases such as "本页展示", "核心观点如下", "具有重要意义", "赋能",
  or "显著提升" unless backed by concrete evidence.
- Bullet openings and sentence lengths vary naturally; avoid same-shape lists.
- No Chinese orphan lines (1–2 chars alone on a line). Proactively rewrite to fix.
- Keep mixed-language tokens intact (English terms, years, percentages, citations).
- Keep headings visually balanced; no tiny trailing second lines.
- Reserve bottom safe area for source notes, page numbers, footer elements.

### Layout Quality
- Prefer content-driven structure over decorative empty cards.
- Prefer strong hierarchy over equal visual weight.
- Use asymmetry intentionally but keep alignment disciplined.
- Keep images intentional with focal crops, contain rules, overlays, or masks.
- Use native charts for simple data but restyle to avoid default office look.
- For tables: shorten, prune, highlight decision-relevant slices first.
- Minimum body text is 18pt; do not fix density by shrinking text.
- For density overflow, split pages or move detail to appendix.

### QA
- Run `ppt office doctor` for Rust outline, issue, and package validation.
- Run `ppt slides-test` or `ppt qa` before delivery.
- Treat out-of-bounds and overlap as separate QA checks.
- Fix overlap, out-of-bounds, font substitution, ugly fallback before delivery.
- Do not skip rendered-slide QA just because the source file "looks right."

## Final Checks

### Core
- [ ] Delivered deck is a real editable `.pptx`
- [ ] `deck.plan.json` rebuilds the delivered deck
- [ ] Slide count matches the plan
- [ ] No tiny-text workarounds on important slides
- [ ] Body text remains at or above 18pt; no readability-critical text is shrunk below 18pt
- [ ] `theme rhythm` passes: no 3 consecutive same-theme slides
- [ ] Review `theme rhythm` for decks with more than 8 slides: consider at least 2 hero pages and both `dark-premium` / `light-editorial` hero treatments when narrative fit allows

### Text
- [ ] Text owner pass completed before layout when the deck content needed more
  than mechanical cleanup
- [ ] No 1–2 character Chinese orphan lines
- [ ] Mixed-language tokens not split awkwardly
- [ ] Titles/subtitles do not end with tiny trailing lines
- [ ] `zh-layout-guard` passes: Chinese titles/subtitles avoid 1-2 char orphan endings
- [ ] Chinese titles are kept in a readable length band or rewritten with semantic line breaks
- [ ] Body content clear of bottom safe area

### Visual
- [ ] Rust inspector check was run
- [ ] Images cropped/contained intentionally
- [ ] Fonts render as intended, no silent substitution
- [ ] Geometry QA checks were run
- [ ] Out-of-bounds check passed via `ppt slides-test --fail-on-overflow`
- [ ] Overlap check passed via rendered review and/or `ppt qa`
- [ ] Rendered slides reviewed through `$visual-review`
- [ ] Design audit verdict is `match` or only acceptable `minor drift` when a
  `DESIGN.md` / visual contract exists
- [ ] Cover, section, and closing slides stay in one style family (`dark-premium` or `light-editorial`)
- [ ] No major slide is mostly decorative empty space
- [ ] Charts/tables look presentation-grade, not default office output
- [ ] Each slide has enough substance to justify its page
- [ ] `anti-ugly` blacklist has zero hits
- [ ] No decorative underline directly under title unless it encodes information
- [ ] No low-contrast gray text on either dark or light canvases
- [ ] No equal-weight card farm with no visual leader
- [ ] No default Office chart palette survives final styling
- [ ] No text placed on image without a solid protection layer
- [ ] No whole-deck isomorphic layout repetition

### Theme Specific
- [ ] Dark theme: body text contrast remains projector-safe
- [ ] Light theme: text hierarchy and separators stay clear on bright backgrounds
