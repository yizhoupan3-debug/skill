# Source Routing

Use this reference when a user wants a product to feel like a named company or
interface before any redesign work starts.

## Goal

Turn vague requests such as "像 Linear 一样" or "给我 Stripe 的品牌 token，再混一点
liquid glass motion" into a stable handoff contract for the real design owner.

## Decomposition Matrix

For each named reference, split the request into five surfaces:

| Surface | Questions | Typical downstream owner |
| --- | --- | --- |
| Brand tokens | Which palette, type rhythm, radii, density, and contrast posture matter? | `frontend-design` / `css-pro` |
| Layout grammar | Is the reference sparse, compact, editorial, bento, dashboard-first, or document-first? | `frontend-design` |
| Component signatures | Which cards, lists, nav shells, buttons, or empty states define the feel? | `frontend-design` |
| Motion language | Are we borrowing restrained transitions, liquid glass motion, or magnetic interactions? | `motion-design` |
| Product tone | Does the product feel clinical, technical, luxurious, calm, or playful? | `frontend-design` |

## Decision Labels

- `verified`: the cue is tightly tied to the named reference and safe to state directly
- `portable`: the cue can be borrowed without making the result feel like a clone
- `risky`: the cue is too signature-heavy and should be adapted rather than copied

## Example Mappings

### Linear

- `verified`
  - compact information density
  - quiet neutrals with precise accent restraint
  - low-noise borders and disciplined spacing
- `portable`
  - command-surface clarity
  - strong list hygiene
  - hierarchy through density instead of decoration
- `risky`
  - literal sidebar / issue-table mimicry
  - exact iconography or token values

### Stripe

- `verified`
  - brand-token discipline
  - polished type hierarchy
  - layered but controlled gradients
- `portable`
  - trustworthy "infrastructure-grade" visual tone
  - elevated card/surface rhythm
- `risky`
  - direct reuse of iconic purple/indigo signatures without adaptation

### Liquid Glass Motion

- `verified`
  - translucent depth
  - motion-led material perception
  - blur plus highlight driven emphasis
- `portable`
  - surface depth hierarchy
  - soft transition language
- `risky`
  - overusing blur or reflection until legibility drops

## Handoff Rule

When the reference frame is clear:

- visual redesign -> `$frontend-design`
- motion execution -> `$motion-design`
- token / layout implementation -> `$css-pro` or `$tailwind-pro`
- screenshot-grounded critique -> `$visual-review`

