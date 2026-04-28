# Checklist Planner Template

Use this compact structure for both new and normalized checklists.

```markdown
# <Task> Checklist

## Goal
- ...

## Current State
- ...

## Execution Points

- [ ] P1: <parallel lane or serial chain title>
  - Current state:
  - Goal:
  - Constraints:
  - Ordered substeps:
  - Deliverables:
  - Acceptance:
  - Stop conditions:
  - Update rule:

- [ ] P2: <parallel lane title>
  - Current state:
  - Goal:
  - Exclusive scope:
  - Forbidden scope:
  - Deliverables:
  - Acceptance:
  - Stop conditions:
  - Update rule:

## Recommended Execution Order
- Parallel:
- Serial:
- Integrator-only updates:

## Overall Acceptance
- ...
```

Rules:

- Keep serial dependencies inside one point.
- Split independent work into separate peer points.
- Do not let peer lanes co-edit shared continuity artifacts.
- Treat checklist updates as part of done-ness.
