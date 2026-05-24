# Beamer Design System

Use this reference when the user cares about slide polish, theme upgrades, or whether the deck looks intentionally designed rather than merely compiled.

## Required Design Components

Treat these as first-class source components, not optional garnish:

- cover slide
- content footline
- semantic color palette
- reusable emphasis boxes
- closing slide such as `Thanks`, `Q&A`, or `Discussion`

If any one of these is missing, call that out explicitly instead of signing off on "overall polish."

## Cover Slide

Do not default to a raw `\titlepage` unless the deck is intentionally austere.

Prefer a cover that establishes hierarchy clearly:

- title as the dominant element
- subtitle or framing line as secondary support
- presenter, affiliation, course, or date grouped in a compact metadata block
- one visual anchor such as a vertical band, accent rule, or background block

Default cover discipline for this skill:

- keep it to two textual layers above the metadata block: title, then subtitle or one framing line
- do not add a third explanatory sentence on the cover unless the user explicitly wants a claim-led opening frame
- keep enough vertical clearance so the plain-frame footer remains fully visible

Common failure modes:

- title, subtitle, and metadata all have similar weight
- too much empty space with no visual anchor
- the default Beamer title page survives unchanged inside an otherwise customized deck

Default house style for this skill:

- center the title inside a large white card or controlled content area
- use one dark navy anchor instead of several unrelated decorations
- place presenter and course or project metadata below the main title block
- if using a shadow, keep it soft and understated

## Footline

Use a consistent footline on content frames. A good default is three zones:

- presenter or team on the left
- short title or section label in the middle
- frame number on the right

Rules:

- keep the height low and stable across frames
- do not let body content sit on top of the footline safe area
- suppress the footline on `[plain]` opening and closing frames only when the absence looks intentional
- for custom plain-frame footers, lift the bar slightly above the paper edge so export or viewer chrome does not visually clip it

Preferred default for this skill:

- navy on the left
- muted blue accent in the center
- navy on the right
- white text, small type, no progress bars

## Color System

Define a small palette with named roles before slide authoring:

- primary: headings, structural accents, footline base
- accent: secondary highlights or alternative category
- highlight: selected key takeaway or result callouts
- neutral: panel backgrounds, table striping
- alert: warnings, losses, rejections, or risk

Avoid these patterns:

- introducing new colors for isolated frames
- using saturated red, green, orange, and blue at equal strength everywhere
- encoding meaning with color alone when bold text or labels are also needed

Default palette bias for this skill:

- primary: deep navy
- accent: muted steel blue
- highlight: warm muted gold used selectively, not sprayed through all structural elements
- neutral: pale blue-gray or near-white panels
- alert: subdued slate rather than bright red unless the task truly needs red

## Emphasis Boxes

Create a small reusable box system instead of ad hoc `tcolorbox` variants.

Recommended roles:

- information box: setup, method, baseline explanation
- highlight box: key insight, contribution, important takeaway
- warning box: risk, limitation, failure case, rejection

Keep these properties consistent:

- padding
- border radius
- rule thickness
- title styling
- contrast level between frame and background

Use the stronger box type sparingly. If every frame uses the highlight box, nothing is highlighted.

In the default house style, boxes should feel like editorial content panels:

- dark header band
- pale body
- light border or no visible border
- minimal corner radius
- no glossy gradients or oversized shadows

For highlight boxes specifically, a good default is:

- warm gold title band with white text
- white body with a thin matching gold border
- tighter padding than ordinary information blocks
- use only where the slide needs one clearly elevated takeaway

Also preserve lateral whitespace:

- body frames should not hug the left paper edge
- a slightly wider text margin is usually safer than squeezing more words per line
- if a diagram needs full width, rebalance that frame locally instead of shrinking the whole deck margins

## Closing Slide

End the deck with a designed closing frame rather than letting the last analytical slide act as the ending.

Good defaults:

- `[plain]` frame
- one strong closing phrase
- small metadata card with presenters or contact info
- optional subtitle line repeating the deck topic

Common failure modes:

- no closing slide at all
- a closing slide that looks visually unrelated to the cover
- a huge `Thank You` floating alone with no supporting structure

Default closing direction for this skill:

- repeat the deep navy header treatment from the body or cover
- keep one discussion panel or metadata panel on the right if useful
- keep the bottom footline or a matching plain-footer treatment so the deck ends in the same system it started with

## Visual Sign-Off Checklist

Before approving a Beamer deck, inspect rendered slides and answer:

- Does the cover look designed, not default?
- Do all non-plain frames share a consistent footline?
- Does the color usage stay within the declared roles?
- Are callout boxes visually consistent and semantically meaningful?
- Does the closing slide feel like a designed end state?
