# Research Harness

Unified research harness crate for the skill framework. Integrates paper revision loop, literature search, claims management, AIGC detection/reduction, verification pipelines, and research logging.

## Modules

| Module | Description |
|--------|-------------|
| `search` | Literature search (Semantic Scholar, arXiv, paperplain MCP bridge) |
| `claims` | Claim ledger management, drift detection, ceiling computation |
| `log` | Research activity logging (SQLite FTS5), knowledge graph |
| `citation` | Citation audit, BibTeX rendering, DOI validation |
| `review` | Multi-round adversarial review orchestration |
| `hooks` | Paper prose/adversarial hooks, research activity log hooks |
| `aigc` | AIGC detection (n-gram, burstiness, syntactic patterns), humanization |
| `verification` | Literature, statistical, prose QC, structure, formal verification |
| `render` | Markdown rendering pipeline |
| `state` | Research state persistence: load/save/migrate/hydrate from YAML/JSON |
| `workspace` | Workspace initialization, file sync, ledger events |
| `text` | Text processing: slugification, XML parsing, content word extraction |
| `provenance` | Git provenance and environment fingerprint capture |
| `smoke` | General-purpose experiment smoke test engine (quick directional probes via templates/ executables) |
| `smoke_cache` | LRU+TTL cache with disk persistence for experiment results |
| `mcp` | MCP tool definitions and dispatch for research tools |
| `mcp_tools` | Research MCP tool dispatch (delegated from host-projection) |
| `proof_dag` | Blueprint-DAG proof architecture (AND-OR DAG) |
| `proof_dag_serialize` | Serialization for Blueprint-DAG |
| `subprocess` | Subprocess execution helpers (timeout, truncation) |
| `types` | Core types: review, claims, AIGC, search, verification |
| `util` | Shared utility functions (novelty gate helpers, arr_mut, str_field) |

## Dependencies

- `core-state`, `core-state-utils`, `core-policy` (leaf crates, no cycle risk)
- `framework-kernel` (L0 kernel utilities)
- `host-projection` (L5 hook dispatch via function pointers)
- ~~`fr-utils`~~ *(merged into `runtime-core`, 2026-07)*
- Common workspace deps (anyhow, chrono, reqwest, rusqlite, serde, regex, ...)

Does **not** depend on `runtime-core` — avoids circular dependency.
`runtime-core` can call `research-harness` hook interfaces through `host-projection` function pointers.

## MCP Tools

Exposed through `host-projection`'s `mcp_stdio_harness`:

### Research tools
- `research_aigc_check` — AIGC detection (0-100 score + signal list)
- `research_claim_drift` — Claim drift detection (original vs current claims)
- `research_literature_search` — Cross-source academic literature search (arXiv, Semantic Scholar)
- `research_review_dimensions` — Get review dimension prompts and checklists
- `research_review_loop` — Multi-round adversarial review loop
- `research_smoke` — General-purpose experiment smoke test engine

### Verification tools
- `research_verification_prose` — Prose QC: terminology consistency, slop detection, hedging analysis
- `research_verification_statistical` — Statistical checks: GRIM test, p-value verification, multiple comparison correction
- `research_verification_literature` — Literature checks: DOI reachability, claim coverage
- `research_verification_structure` — Structure checks: LaTeX compilation, figure reference consistency
- `research_verification_reproducibility` — Reproducibility checks: seed, determinism, environment, data versioning
- `research_verification_formal` — Formal verification: dimensional analysis
- `research_aigc_humanize` — AIGC reduction (syntactic rewriting / lexical substitution). **Note: not yet implemented as an MCP tool.**

### Math verification tools
- `math_asymptotic_estimate` — Asymptotic magnitude estimation
- `math_asymptotic_chain` — Asymptotic chain verification
- `math_proof_dag_init` — Initialize proof DAG
- `math_proof_dag_decompose` — Decompose proof node into sub-goals
- `math_proof_dag_verify` — Verify proof DAG structural completeness
- `math_proof_dag_status` — View proof DAG progress summary
- `math_sympy_verify` — Symbolic identity verification
- `math_sympy_simplify` — Expression simplification
- `math_prove_inequality` — Inequality proving via SMT solver
- `math_backend_available` — Check Z3/SymPy/Lean backend availability
- `math_lean_verify` — Theorem formalization via Lean

### QG Route Checkers (registered via RUNTIME_REGISTRY.json)

| Checker | Scene | Description |
|---------|-------|-------------|
| `Asymptotic` | RESEARCH | Asymptotic relation verification |
| `DimensionalConsistency` | RESEARCH | Dimensional analysis |
| `Inequality` | RESEARCH | Linear inequality feasibility via minilp |
| `Literature` | RESEARCH | Citation-claim alignment, DOI reachability |
| `ProseQCChecker` | RESEARCH | AI slop detection, hedging analysis, terminology consistency |
| `Reproducibility` | RESEARCH | Experiment seed/determinism/environment lock |
| `StatisticalChecker` | RESEARCH | p-value recomputation, GRIM test, effect size |
| `Structure` | RESEARCH | LaTeX compilation, cross-ref consistency |
| `Symbolic` | RESEARCH | Symbolic identity proving, growth classification |
| `SympyBridge` | RESEARCH | SymPy symbolic verification bridge |

## Backward Compatibility

- `research-harness` remains as an independent binary (thin CLI wrapper pending)
- `host-projection` hook registration can be incrementally migrated to `research_harness::hooks`
- All existing MCP tool names remain unchanged; callers are unaffected

## Building

```bash
cargo build -p research-harness
```

## Testing

```bash
cargo test -p research-harness
```

## License

MIT
