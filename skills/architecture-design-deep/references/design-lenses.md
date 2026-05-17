# Design review lens catalog (candidate dimensions for architecture & design)

Use inside `architecture-design-deep` as **optional** modes or parallel read-only lanes—not separate top-level owners. **Pick** lenses from this catalog for the current scope (component, service boundary, system integration, deployment topology); do **not** assume every review runs every row unless the user explicitly asks for full-dimensional / exhaustive-all-lenses coverage.

For each lens you **select**, work **systematically** within that lens (multiple failure modes, scaling axes, abuse cases, edge paths). For lenses you **omit**, briefly cover why only in **full report profile**; in **compact**, optional **one** line **`Out of scope:`** is allowed **only immediately after** a **`Scope:`** line (see main [`SKILL.md`](../SKILL.md) **Compact envelope**). **Without** `Scope:`, fold omission rationale into the first finding or the single `Scope:` line—do **not** lead with standalone `Out of scope:`.

## Core lenses (typical architecture & design review)

- **Architecture correctness**: consistency between stated architecture and actual component wiring; layering violations; dependency direction violations; implicit circular dependencies; architectural invariants that the implementation must satisfy.

- **Design consistency**: patterns used consistently across the codebase; abstraction levels appropriate for each layer; single-responsibility adherence within components; DRY vs. necessary duplication trade-offs.

- **Component boundaries**: coupling between modules (afferent/efferent coupling); cohesion within modules; interface surface area (is the API minimal and intentional); information hiding (are internals leaked); boundary integrity (can external code bypass intended abstractions).

- **Data flow & ownership**: ownership and mutation paths; serialization boundaries; consistency guarantees (strong vs. eventual); data lifecycle (creation, mutation, archival, deletion); shared mutable state risk.

- **Extensibility / evolution**: how the design supports future changes without rewriting; extension points vs. hard-coded paths; versioning strategy for APIs and data; backward compatibility accommodations; feature-flag or toggling readiness.

- **Resilience & failure domains**: failure domains and blast radius; recovery paths (graceful degradation, retry, fallback); bulkheading between components; state consistency under partial failure; timeout and circuit-breaker adequacy.

## Optional lenses (use when scope warrants)

- **Over-engineering**: unnecessary abstraction layers; speculative generality without demonstrated need; framework lock-in where simpler constructs suffice; premature optimization adding complexity without measured benefit.

- **Under-engineering**: missing boundaries where modularity is warranted; monolithic growth without separation of concerns; insufficient abstraction for the problem's complexity; copy-paste reuse instead of shared mechanism.

- **Technology fit**: architectural assumptions that conflict with chosen tech stack; impedance mismatch between problem domain and solution structure; operational model (stateful vs. stateless, sync vs. async) misaligned with infrastructure constraints.

- **Operational & observability**: deployability (how is this shipped and configured); monitoring coverage (can key metrics and failure modes be observed); debuggability (can production issues be traced to root cause); cost model (resource usage under load).

- **Security architecture**: trust boundaries between components; authentication/authorization flow correctness; data protection at rest and in transit; secret management; supply chain integrity; least-privilege principle.

Default visible output stays **severity-sorted findings** in main [`SKILL.md`](../SKILL.md), governed by **Compact envelope** there (no tables/summary headings before `[P*` / `Caveat:`); exhaustive lens reasoning is internal—**do not** default to dropping a lens **summary grid** ahead of findings. **Caveat** rows may use **`[P2]`** or a single **`Caveat:`** line per that envelope. Verdict stays optional **one line** **after** findings in compact mode; lens grouping applies only in **full report profile**.
