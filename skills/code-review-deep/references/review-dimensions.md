# Review lens catalog (candidate dimensions)

Use inside `code-review-deep` as **optional** modes or parallel read-only lanes—not separate top-level owners. **Pick** lenses from this catalog for the current scope (PR slice, file, module, service); do **not** assume every review runs every row unless the user explicitly asks for full-dimensional / exhaustive-all-lenses coverage.

For each lens you **select**, work **systematically** within that lens (multiple failure modes, abuse cases, edge paths). For lenses you **omit**, say why briefly in the **Scope / Lenses / Omitted** preamble (see main `SKILL.md`).

## Core lenses (typical code-change review)

- **Correctness**: logic, error paths, concurrency, lifetimes, invariants, flaky tests.
- **Security**: trust boundaries, injections, deserialization, secrets, `unsafe`/FFI.
- **API / ABI & compat**: public surfaces, versioning, semantic vs doc drift, rollout hazards.
- **Deps / supply-chain**: advisories, CVE reachability, pins, licensing, update risk.
- **Observability**: logs, metrics, traces, production debuggability.

## Optional lenses (use when scope warrants)

- **First principles & subtraction**: unnecessary abstraction, duplicate sources of truth, speculative wrappers/fallbacks, scope creep beyond the stated goal; whether a smaller change would satisfy the same invariant (language-agnostic).
- **Dead-code signals**: symbols/modules with no reachable references, broad `dead_code` / unused allowances, duplicate “islands” of logic, unreachable paths—**report only** in review posture; confirm with the project’s build/test/search tooling when possible.
- **Stale docs & truth drift**: README/architecture docs contradicting behavior, broken cross-references, versioned API docs out of sync with code, onboarding text that no longer matches repo layout.

Return **verdict first**, then findings grouped by **the lenses you actually applied**.
