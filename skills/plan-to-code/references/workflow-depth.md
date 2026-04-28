# Detailed Implementation Workflow

This document provides the full 7-phase workflow for `plan-to-code`. Use this for complex, multi-layer implementations (Structured Path) or when high-fidelity execution maps are required.

## Phase 1: Parse and classify the brief

- Read the plan or spec file first.
- Separate explicit requirements, implied requirements, constraints, and deliverables.
- Translate broad statements into concrete engineering work: data model, API surface, UI states, background jobs, config, tests, and release impact.
- Use [input-maturity-levels.md](input-maturity-levels.md) to classify the brief before deciding implementation depth.
- If the brief is thin, infer from nearby code, naming conventions, and existing patterns before asking the user.
- Decide the execution mode using [execution-modes.md](execution-modes.md).

## Phase 2: Inspect the repository before coding

- Find the relevant entrypoints, modules, routes, services, schemas, and tests.
- Use fast search (`rg`, `rg --files`) and read only the files needed to understand the implementation path.
- Check whether similar features already exist and copy the established pattern when it reduces risk.
- Audit partial implementation first when stub files, TODOs, or half-finished work already exist.
- If the work is large or risky, coordinate with `$gitx`.

## Phase 3: Build an execution map

- Decide what must change for the feature to be real, not nominal.
- Cover the full path from input to observable behavior: storage, logic, APIs, UI, validation, errors, and tests.
- Use [completeness-checklist.md](completeness-checklist.md) for cross-layer work.
- Split work into separately verifiable execution slices.
- Identify which slices can be delegated after consulting `$runtime delegation gate`.
- Define each slice using the template in [delegation-prompts.md](delegation-prompts.md).

## Phase 4: Implement end to end

- Edit files directly in the repository.
- Preserve existing conventions for framework, state management, error handling, naming, and tests.
- Wire every touched layer together so the feature is runnable after the change.
- Prefer small, reviewable implementation slices with checkpoints instead of one giant patch.

## Phase 5: Close the gap between "works" and "complete"

- Add missing glue: config keys, migrations, types, exports, route registration, and permissions.
- Read [common-missed-work.md](common-missed-work.md) before concluding.
- Handle obvious edge cases and avoid mock-only code paths.

## Phase 5.5: Self-Reflection & Internal Audit

- **Critique own work**: Before concluding implementation, act as a strict reviewer.
- **Find "Blind Spots"**: Search for unhandled errors, unwired UI states, or potential race conditions.
- **Reinforce**: If the user requested "Superior Quality", proactively trigger `$runtime verification gate` findings and fix them before concluding.

## Phase 6: Review

- **Spec Review**: Check for missing requirements, scope creep, and unwired behavior.
- **Quality Review**: Naming, maintainability, and local pattern fit.
- Use prompts from [delegation-prompts.md](delegation-prompts.md) for explicit reviewer passes.

## Phase 7: Verify

- Run targeted validation: tests, typecheck, lint, or focused manual checks.
- Use [verification-matrix.md](verification-matrix.md) to choose the minimum credible verification set.
- Fix failures caused by the implementation before concluding.
