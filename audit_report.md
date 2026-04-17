# PPTX Skill Audit Report

Date: 2026-04-15
Target: `skills/ppt-pptx`
Mode: local-supervisor audit
Verdict: PASS

## Acceptance Contract

- `ppt-pptx` must support a clean authoring path from template or outline to real editable `.pptx`.
- The generated deck must survive rendered QA, overflow checks, and structure extraction.
- Missing sample images must not crash first-run bootstrap.
- Existing deck lane boundaries must remain clear: source-first rebuild stays in `ppt-pptx`, in-place edits stay in `officecli`.

## Evidence Summary

- `outline_to_deck.js --help` now prints usage and exits cleanly.
- Freshly regenerated outline/template/sample-deck outputs now request `Arial` as the authored font family.
- Outline flow passed:
  - `outline.yaml -> deck.js -> deck.pptx`
  - render succeeded
  - overflow test passed
  - font audit reported no issues
  - structure extraction succeeded
- Template flow passed:
  - `deck.template.js -> deck.pptx`
  - render succeeded
  - overflow test passed
  - font audit reported no issues
- Example `deck.js` passed:
  - `deck.js -> deck.pptx`
  - render succeeded

## Fixes Applied

1. Hardened `outline_to_deck.js`
   - added `--help` handling
   - fixed `totalSlides` to use reflowed slide count
   - skipped dominant-color extraction when cover image is absent
   - generated decks now tolerate missing optional images
2. Hardened `assets/deck.template.js`
   - template no longer crashes when sample images are absent
   - fixed fallback branch to avoid duplicate overlay/label overlap
3. Hardened sample `deck.js`
   - sample deck now builds without bundled image files
4. Repaired docs
   - `references/install.md` now includes `js-yaml`
   - install/workflow docs now explain optional image fallback behavior
5. Added repeatable regression entrypoint
   - `scripts/smoke_test.py` now codifies the outline/template/sample-deck authoring checks
   - `package.json` now exposes `npm run smoke:test`
6. Enforced cross-platform font defaults
   - default authored sans-serif changed to `Arial`
   - default authored monospace changed to `Courier New`
   - platform-specific defaults were removed from templates and helper typography

## Residual Risks

- Fallback placeholder panels are structurally valid but not presentation-final; real decks should still supply local images before delivery.
- The audit did not run `$visual-review`; rendered PNGs were verified by script-based checks only.
