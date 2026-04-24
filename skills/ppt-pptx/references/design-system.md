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
assets. Use `$frontend-design` when no source style exists but the deck still
needs a fresh premium visual direction. After rendering, use `$visual-review`
for image-grounded findings and `$design-output-auditor` for the final drift
verdict against this contract.

Map those fields into `deck.plan.json`: palette roles, typography scale, panel/card
styles, cover/section/closing grammar, chart styling, and banned drift.

Keep the contract actionable. A good deck plan should name the palette roles,
font families, recurring slide components, and layout families clearly enough
that the Rust builder can regenerate the deck without guessing.

## Default Aesthetic

Prefer a black-luxury direction:

- pure black or near-black stage as the base
- a cover led by one large blurred background image
- strong white typography and protected text zones
- charcoal information panels instead of bright, noisy card farms
- restrained electric-blue glow instead of loud gradients or many accent colors
- image modules that feel built into the page structure rather than dropped on top

Avoid seminar stiffness, default office chart palettes, random icon stickers, equal-weight card mosaics, generic keynote glassmorphism, and dark slides with weak unreadable text.

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

- cover: blurred full-slide background image plus a dark information stack
- content slides: black base plus 2-4 charcoal panels with clear hierarchy
- section slides: short title stack, a dominant image or evidence band, and one supporting module group
- image-data slides: one wide dark evidence surface plus a small number of metric chips
- closing slide: same family as the cover, quieter and more cinematic than informational

When the outline is dense, split slides by idea before shrinking text. A slightly
longer deck with readable hierarchy is better than a compressed deck that only
works at editor zoom.

## Slide Roles

### Cover

- one focal point only
- title is the hero
- subtitle is short
- metadata is compact and visibly secondary
- cover image should already be softened before it reaches the slide
- never let the background image compete with the title stack
- add one structural anchor: glow line, protected info slab, small focus card, or asymmetrical dark block

### Content Slide

- one main message
- 2-4 information zones
- one clear reading path
- supporting labels should be quiet, not ornamental

### Data Slide

- make the comparison obvious before showing detail
- reduce chart clutter
- color only what matters
- if a table is too wide, it is a structure problem, not a font-size problem

### Closing Slide

- same family as the cover
- one closing claim or prompt
- do not end on an accidental leftover evidence slide

## Hierarchy

Use four levels only:

1. title
2. section / takeaway
3. body
4. note / source

If more levels appear, the slide usually becomes muddy.

Use the classic four layout relationships consistently:

- contrast: clear difference between title, body, and annotation
- alignment: elements snap to a visible grid or axis
- proximity: related content sits together instead of drifting apart
- repetition: recurring labels, captions, and cards share the same visual grammar

## Composition Patterns

Preferred patterns:

- full-slide blurred hero plus protected title stack
- wide dark evidence band plus metric chips
- left narrative stack plus right image surface
- one dominant image plus one concise takeaway panel
- dark panel grid with one clear visual focal point

Use equal columns only when the content is genuinely balanced.

For editable PPTX output, prefer shapes, text, and simple structured chart/table
data over raster-only composites. Use raster assets for visual evidence,
screenshots, or image-led slides, not for text that collaborators need to edit.

## Image Embedding

High-end slides do not simply "insert an image". They embed it into the composition.

Preferred image treatments:

- full-slide cover background that has already been blurred or strongly softened
- framed image panel with a controlled dark overlay
- image block that locks to the page edge while text sits on a protected solid zone
- dark image band with stat pills or labels sitting inside the composition
- one small sharp focus image over a blurred large background when the cover needs extra depth

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

- every card with the same weight
- shadow on everything
- title centered above a mostly empty slide
- tiny text inside giant containers
- multiple unrelated visual motifs in one deck
- charts pasted in with their default palette
- cover and closing slide looking like different templates
- beautification before structure is clear
- full-image slides with no visual focus or text placement logic
- tables and charts kept in their default software styling
- dark backgrounds with weak low-contrast text
- raw unblurred cover images used directly behind dense text
- dark decks that confuse "cinematic" with "hard to read"

## Design Audit Verdicts

For rendered review, use the same verdict language as `$design-output-auditor`:

- `match`: output follows the declared deck design system.
- `minor drift`: small spacing, emphasis, or tone issue that does not change the
  visual identity.
- `material drift`: palette, typography, density, or layout grammar no longer
  matches the contract.
- `hard fail`: AI-slop, default Office styling, unreadable contrast, or an
  anti-pattern relapse that breaks the deck's intended identity.
