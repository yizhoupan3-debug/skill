# Review dimensions

Use inside `code-review-deep` as modes or parallel read-only lanes—not separate top-level owners.

- **Correctness**: logic, error paths, concurrency, lifetimes, invariants, flaky tests.
- **Security**: trust boundaries, injections, deserialization, secrets, `unsafe`/FFI.
- **API / ABI & compat**: public surfaces, versioning, semantic vs doc drift, rollout hazards.
- **Deps / supply-chain**: advisories, CVE reachability, pins, licensing, update risk.
- **Observability**: logs, metrics, traces, production debuggability.

Return **verdict first**, then the issues most likely to **block merge** or cause **incidents**.
