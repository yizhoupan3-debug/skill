# DESIGN.md Format Reference

Use this when the task requires a concrete `DESIGN.md` structure.

## Structure

A `DESIGN.md` file has two layers:

- YAML front matter at the top for machine-readable tokens.
- Markdown body for human-readable rationale, application guidance, and guardrails.

Tokens are normative. Prose explains why the tokens exist and how to apply
them when the token set does not answer a design decision directly.

## Minimal Token Schema

```yaml
---
version: alpha
name: Product Visual Identity
description: Short description of the product mood and audience.
colors:
  primary: "#1A1C1E"
  secondary: "#6C7278"
  accent: "#B8422E"
  surface: "#F7F5F2"
  on-surface: "#1A1C1E"
typography:
  headline-lg:
    fontFamily: Public Sans
    fontSize: 48px
    fontWeight: 600
    lineHeight: 1.1
    letterSpacing: -0.02em
  body-md:
    fontFamily: Public Sans
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 32px
rounded:
  sm: 4px
  md: 8px
  lg: 16px
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.surface}"
    typography: "{typography.body-md}"
    rounded: "{rounded.sm}"
    padding: 12px
---
```

## Section Order

Use `##` headings in this order when present:

1. `Overview`
2. `Colors`
3. `Typography`
4. `Layout`
5. `Elevation & Depth`
6. `Shapes`
7. `Components`
8. `Do's and Don'ts`

Unknown sections may be preserved, but duplicate core sections should be fixed.

## Capture Checklist

- `Overview`: product personality, audience, density, emotional target.
- `Colors`: semantic roles, contrast expectations, accent discipline.
- `Typography`: display/body/label roles, scale, weights, casing.
- `Layout`: grid, container width, spacing rhythm, responsive behavior.
- `Elevation & Depth`: shadows, tonal layers, borders, blur, or flat hierarchy.
- `Shapes`: radius scale and shape language.
- `Components`: buttons, cards, inputs, nav, chips, dialogs, tables, states.
- `Do's and Don'ts`: specific guardrails that prevent style drift and AI-generic output.

## Validation

If Node/npm access is acceptable:

```bash
npx @google/design.md lint DESIGN.md
npx @google/design.md diff DESIGN.before.md DESIGN.md
```

Use lint findings as structured evidence, not as a replacement for visual
judgment. For rendered fidelity, route to `visual-review`.

## Implementation Mapping

When applying `DESIGN.md` to code, map before editing:

- `colors.*` -> CSS variables, Tailwind `theme.colors`, or component theme colors.
- `typography.*` -> font family, size, weight, line height, letter spacing, and casing utilities.
- `spacing.*` -> spacing scale, grid gaps, container padding, section rhythm.
- `rounded.*` -> radius scale for buttons, cards, dialogs, inputs.
- `components.*` -> specific component variants and states.

After mapping, implementation belongs to the narrowest owner such as
`tailwind-pro`, `css-pro`, `frontend-design`, or a document/deck artifact skill.
