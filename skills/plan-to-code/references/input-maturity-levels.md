# Input Maturity Levels

Classify the user's brief before deciding implementation depth, assumptions, and how much design work Codex must supply.

## Level 1: Idea

Signals:

- Short request with intent but little structure
- Few or no acceptance criteria
- Vague phrases like "做一个", "搞个页面", "支持一下这个功能"

Default behavior:

- Infer a minimal but coherent scope from the repository.
- Prioritize one complete user flow over broad feature surface.
- Add only obvious supporting work needed to make the flow usable.
- Keep new abstractions minimal.

Ask a blocking question only if:

- The feature could land in multiple unrelated product areas.
- The change could destroy or overwrite important existing behavior.

## Level 2: PRD

Signals:

- Has goals, user stories, feature list, or UI/API expectations
- Mentions states, roles, business rules, or acceptance criteria
- May still omit technical structure

Default behavior:

- Implement the stated feature set, not just one demo path.
- Fill in technical decisions from existing architecture.
- Cover happy path, validation, empty state, error state, and access rules when implied.
- Add tests around the most important product behaviors.

Ask a blocking question only if:

- A key business rule is contradictory.
- Required data or permissions cannot be inferred safely.

## Level 3: Technical Spec

Signals:

- Defines architecture, modules, APIs, schemas, or sequence details
- Names tables, routes, events, jobs, or component boundaries
- Includes explicit non-functional constraints

Default behavior:

- Implement closely against the spec unless it clearly conflicts with the repository.
- Treat omitted glue work as still required: wiring, exports, config, migrations, validation, tests.
- Surface meaningful deviations if the codebase forces a different approach.

Ask a blocking question only if:

- The spec conflicts with critical repository constraints and multiple interpretations are plausible.

## Level 4: Partial Implementation

Signals:

- Existing branch, stub files, TODOs, partial routes, draft components, or incomplete tests already exist
- User asks to "补齐", "接上", "做完整", or "把半成品做完"

Default behavior:

- Audit what already exists before writing new structure.
- Reuse viable partial work and replace weak scaffolding where needed.
- Focus on integration gaps, regressions, and missing finish work rather than re-architecting by default.

Ask a blocking question only if:

- Existing partial work and the brief point in materially different directions.

## Depth Policy

Use the maturity level to set the default finish bar:

- Level 1: deliver a narrow but real feature slice.
- Level 2: deliver the stated capability set with product-ready states.
- Level 3: deliver spec fidelity plus repository-compatible glue work.
- Level 4: deliver completion, integration, and stabilization of the unfinished work.

Do not use low maturity as an excuse to ship fake code. Even Level 1 should still be runnable within its chosen slice.
