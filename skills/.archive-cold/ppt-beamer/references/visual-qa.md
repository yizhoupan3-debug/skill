# Visual QA

Use this reference when the deck compiles but may still look wrong.

## Why Visual QA Is Mandatory

Beamer logs catch some failures, but they miss many presentation failures that matter in practice:

- `columns` content that overlaps after a small text edit
- `tcolorbox` bodies that visually collide even though the frame still compiles
- bottom-edge clipping caused by a figure, footnote, or display math block
- captions or source notes that technically exist but are no longer readable
- frames that are formally valid yet visually sparse or unbalanced

Do not sign off on a deck until rendered slide images have been reviewed.
Use `$visual-review` for that review so the overlap check has an explicit verdict and evidence trail, not just a vague visual pass.

## Minimum QA Loop

1. Compile the deck.
2. Run `check_beamer_log.sh` to collect obvious warnings.
3. Render slide PNGs with `render_slides.sh`.
4. Build a review list with `find build/slides-pages -name '*.png' | sort` if the slide count is large.
5. Call `$visual-review` on the rendered PNGs or gallery screenshots. Pass:
   - `target issue`: overlap, clipping, unreadable text, weak hierarchy, or render bug
   - `artifact scope`: page numbers or slide ids
   - `decision lens`: sign-off gate
   - `output need`: `E. Targeted audit`
6. Fix the TeX source and repeat.

The overlap check is mandatory:

- text over text
- box over box
- figure over caption
- body over footline
- equation over surrounding content

## What To Check On Every Frame

- title does not wrap awkwardly or collide with the body
- no title, caption, or bullet ends with a tiny trailing line or one-or-two-character Chinese widow
- body content stays inside the visible safe area
- no box borders or text baselines overlap
- no figure or table touches the footline
- body text remains readable without zooming
- columns align intentionally instead of drifting by accident
- mixed-language tokens, units, dates, and citation markers stay visually intact
- source notes and qualifiers are present where needed
- the frame has enough substance to justify its existence

When a page fails, keep the finding reusable:

- `Verdict:` confirmed / likely / not found / indeterminate
- `Issue:` one short defect label
- `Evidence:` what is visibly wrong
- `Location:` which page and region
- `Impact:` why it matters
- `Confidence:` high / medium / low

## Hidden Failure Patterns

### 1. Silent Bottom Clipping

Common causes:

- `\vspace` accumulates after a block
- an image uses `height=...` too aggressively
- a long equation or table consumes more vertical room than expected

Typical fixes:

- reduce vertical whitespace before shrinking text
- split the frame
- move detail into appendix or speaker notes

### 2. Column Drift

Common causes:

- left and right columns have mismatched internal boxes
- one side contains a display equation or taller block than expected

Typical fixes:

- force top alignment with `[T]`
- simplify the denser column
- switch to stacked layout if both sides are equally dense

### 3. Decorative Boxes That Swallow Space

Common causes:

- too many block containers
- equal-height visual boxes without enough content

Typical fixes:

- remove one container layer
- merge related content
- replace a decorative box with a highlighted sentence or takeaway strip

### 4. Diagram Over-Compression

Common causes:

- diagram is squeezed to preserve a single-frame narrative
- labels are too verbose

Typical fixes:

- shorten node labels
- move commentary outside the diagram
- split one complex figure into two simpler frames

### 5. Dangling Title Tails And CJK Widows

Common causes:

- a title, caption, or bullet is slightly too long for the chosen width
- a block is narrow enough that only one or two Chinese characters fall onto the next line
- mixed-language content forces an awkward break near numbers or English terms

Typical fixes:

- rewrite the line rather than accepting the wrap
- widen the relevant block or rebalance columns
- add a manual break only after exhausting cleaner layout fixes

### 6. Footer Safe-Area Intrusion

Common causes:

- source notes were added after the frame was already full
- a table, figure, or equation was sized to the visible edge instead of the usable safe area
- footline height changed after body spacing was tuned

Typical fixes:

- reclaim vertical whitespace above the bottom zone
- shorten the source line or move detail into `sources.md`
- split the frame or simplify the lowest content block

## Sign-Off Standard

A deck is ready only when:

- the build succeeds
- the warnings are understood
- the rendered pages have been reviewed through `$visual-review`
- overlap has been explicitly checked through `$visual-review`
- no key frame depends on tiny text or luck to remain legible
