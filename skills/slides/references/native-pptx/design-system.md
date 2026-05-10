# PPTX Design System

Use this reference when the deck needs stronger aesthetics, clearer hierarchy, or a more memorable visual identity.

## Deck DESIGN.md Contract

When a deck has source visuals, brand examples, or will go through multiple
design rounds, treat `DESIGN.md` as the design source of truth. Align it with
the shared design skills and include:

- `Visual Theme & Atmosphere`
- `Color Palette & Roles`
- `Typography Rules`
- `Component Stylings`
- `Layout Principles`
- `Slide Component Signatures`
- `Prompt Block For Reuse`
- `Generation Guardrails`
- `Anti-Patterns`

Use `$design-md` to extract these fields from old decks, screenshots, or brand
assets, or to define a fresh premium visual direction when no source style
exists. After rendering, use `$visual-review` for image-grounded findings and
`$design-md` for the final drift
verdict against this contract.

Map those fields into `deck.plan.json`: palette roles, typography scale, panel/card
styles, cover/section/closing grammar, chart styling, and banned drift.

Keep the contract actionable. A good deck plan should name the palette roles,
font families, recurring slide components, and layout families clearly enough
that the Rust builder can regenerate the deck without guessing.

## Theme System

Support two first-class themes. Choose one at deck start and keep it consistent
across cover, content pages, section slides, and closing:

### `dark-premium`

- near-black or charcoal canvas with high-contrast type
- protected text zones over imagery; softened/blurred imagery allowed
- restrained accent highlights and low-noise panels
- cinematic tone without sacrificing readability

### `light-editorial`

- white or warm-light canvas with strong typographic rhythm
- clear sectioning via rules, panels, and spacing instead of heavy shadows
- restrained dark text and one controlled accent family
- editorial tone that stays clean under projector brightness

Avoid seminar stiffness, default office chart palettes, random icon stickers,
equal-weight card mosaics, generic glassmorphism, and low-contrast text in both
themes.

### Theme rhythm rule

Apply `theme rhythm` with one hard constraint and one recommendation:

- Never allow 3 consecutive slides with the same theme family treatment.
- When deck length is greater than 8 slides, it is recommended to include at least 2 hero pages and cover both `dark-premium` and `light-editorial` hero treatments when narrative fit allows.
- If rhythm quality is weak, split/retime section transitions before touching typography density.

## Cross-Platform Font Policy

For decks that should render predictably on both macOS and Windows:

- default sans serif: `Arial`
- default monospace/code font: `Courier New`
- do not hardcode platform-specific defaults such as `Helvetica Neue`, `Calibri`, `Calibri Light`, or `Consolas`

Note:

- For Chinese text, renderer fallback may still differ by platform.
- The policy is to keep the authored family cross-platform-safe and let CJK glyph fallback happen only where necessary, instead of anchoring the whole deck to a Mac-only or Windows-only face.

## Default Layout Bias

For a generic but high-quality PPTX deck, bias toward:

- cover: one focal visual + protected title stack, adapted to the selected theme
- content slides: 2-4 structured zones with clear hierarchy and enough white space
- section slides: short title stack, one dominant evidence module, one support module
- image-data slides: one wide evidence surface plus a small number of metric chips
- closing slide: same family as the cover, quieter and more conclusive than informational

When the outline is dense, split slides by idea before shrinking text. A slightly
longer deck with readable hierarchy is better than a compressed deck that only
works at editor zoom.

## Slide Roles

### Cover

- one focal point only
- title is the hero
- subtitle is short
- metadata is compact and visibly secondary
- if an image is used, soften/protect it before placing dense text
- never let background treatment compete with the title stack
- add one structural anchor: glow line, protected info slab, small focus card, or asymmetrical dark block

### Content Slide

- one main message
- 2-4 information zones
- one clear reading path
- supporting labels should be quiet, not ornamental
- typography and contrast must stay readable at normal projection distance

### Data Slide

- make the comparison obvious before showing detail
- reduce chart clutter
- color only what matters
- if a table is too wide, it is a structure problem, not a font-size problem

### Closing Slide

- same family as the cover
- one closing claim or prompt
- do not end on an accidental leftover evidence slide
- keep visual closure aligned with the chosen theme (`dark-premium` or `light-editorial`)

## Hierarchy

Use four levels only:

1. title
2. section / takeaway
3. body
4. note / source

If more levels appear, the slide usually becomes muddy.

Body text floor:

- Minimum body text is 18pt across normal content slides.
- Density overload must be solved by slide splitting, hierarchy pruning, or appendix migration, never by shrinking below 18pt.

Use the classic four layout relationships consistently:

- contrast: clear difference between title, body, and annotation
- alignment: elements snap to a visible grid or axis
- proximity: related content sits together instead of drifting apart
- repetition: recurring labels, captions, and cards share the same visual grammar

## Composition Patterns

Preferred patterns:

- full-slide or framed hero plus protected title stack
- wide evidence band plus metric chips
- left narrative stack plus right image surface
- one dominant image plus one concise takeaway panel
- panel grid with one clear visual focal point

Use equal columns only when the content is genuinely balanced.

For editable PPTX output, prefer shapes, text, and simple structured chart/table
data over raster-only composites. Use raster assets for visual evidence,
screenshots, or image-led slides, not for text that collaborators need to edit.

## Image Embedding

High-end slides do not simply "insert an image". They embed it into the composition.

Preferred image treatments:

- full-slide or framed cover background already blurred/softened when needed
- framed image panel with a controlled protection overlay
- image block that locks to the page edge while text sits on a protected solid zone
- image evidence band with stat pills or labels inside the composition
- one small sharp focus image over a softened large background when the cover needs extra depth

Text clarity rules:

- never place key text directly on a complex image without a protection layer
- prefer strong dark overlays or solid text slabs over weak semi-transparent gray boxes
- image captions should feel attached to the image, not stranded elsewhere on the slide
- if the image is strong enough to dominate, reduce nearby decoration rather than competing with it
- on dark slides, body text should still read cleanly on a projector; subtle is not an excuse for poor contrast

## Color Discipline

- one primary family
- one accent
- one muted support tone
- one light or dark canvas

Do not add a new bright color every time a slide feels weak.

## Anti-Patterns

- rainbow accents on one slide: each column, node, or card gets a different bright outline, circle, or badge—prefer one lead accent and neutrals; signal sequence with weight, spacing, or a single muted secondary
- diagram glued to photo: flat primitive shapes plus connectors sitting above or beside an unrelated full-bleed photo without shared grid, margin rhythm, or matched white balance—split into two slides or rebuild as one composed surface (hero image with protected caption slab, or diagram-only editorial canvas)
- spreadsheet-on-slide: oversized table with colored header row and zebra fills but no clear headline takeaway or row hierarchy—editorial typesetting with one emphasized row/column, or move detail to appendix
- decorative title underline/rule with no information function
- low-contrast gray body text on dark/light surfaces
- every card with the same weight
- shadow on everything
- title centered above a mostly empty slide
- tiny text inside giant containers
- multiple unrelated visual motifs in one deck
- charts pasted in with their default palette
- text on image without a solid readability protection layer
- cover and closing slide looking like different templates
- beautification before structure is clear
- full-image slides with no visual focus or text placement logic
- tables and charts kept in their default software styling
- dark backgrounds with weak low-contrast text
- raw unblurred cover images used directly behind dense text
- dark decks that confuse "cinematic" with "hard to read"
- whole-deck isomorphic layout repetition with no role variation

## zh-layout-guard

Use this guard for Chinese and mixed-language typography:

- Do not allow Chinese title/subtitle lines ending with 1-2 orphan characters.
- Keep mixed-language tokens unbroken (e.g., `A/B test`, `2026`, `15%`, citation keys).
- Prefer Chinese title length around 8-18 Han characters; for longer titles, rewrite or insert semantic line breaks.

## Design Audit Verdicts

For rendered review, use the shared `$design-md` verdict language:

- `match`: output follows the declared deck design system.
- `minor drift`: small spacing, emphasis, or tone issue that does not change the
  visual identity.
- `material drift`: palette, typography, density, or layout grammar no longer
  matches the contract.
- `hard fail`: AI-slop, default Office styling, unreadable contrast, or an
  anti-pattern relapse that breaks the deck's intended identity.
