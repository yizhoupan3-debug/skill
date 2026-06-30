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
| `smoke` | Smoke tests for academic source freshness |

## Dependencies

- `core-state`, `core-state-utils`, `core-policy` (leaf crates, no cycle risk)
- `framework-kernel` (L0 kernel utilities)
- `host-projection` (L5 hook dispatch via function pointers)
- `fr-utils` (constants, types)
- Common workspace deps (anyhow, chrono, reqwest, rusqlite, serde, regex, ...)

Does **not** depend on `runtime-core` — avoids circular dependency.
`runtime-core` can call `research-harness` hook interfaces through `host-projection` function pointers.

## MCP Tools & QG Checkers

Exposed through `host-projection`'s `mcp_stdio_harness`:

- `research_review_dimensions` — Get review dimension prompts and checklists
- `research_aigc_check` — AIGC detection (0-100 score + signal list)
- `research_aigc_humanize` — AIGC reduction (syntactic rewriting / lexical substitution)
- `research_claim_drift` — Claim drift detection (original vs current claims)
- `research_review_loop` — Multi-round adversarial review loop

### QG Route Checkers (Wave 4b/5b)

Registered via `RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations` into the shared `CheckerRegistry`
at startup. All checkers are in-place adapter modules in `src/verification/`:

| Checker | Scene | Description |
|---------|-------|-------------|
| `LiteratureGate` | RESEARCH | Citation-claim alignment, DOI reachability, closest-work identification |
| `ProseQCChecker` | RESEARCH | AI slop detection, hedging analysis, terminology consistency |
| `Reproducibility` | RESEARCH | Experiment seed/determinism/environment lock verification |
| `StatisticalChecker` | RESEARCH | p-value recomputation, GRIM test, effect size reporting |
| `StructureGate` | RESEARCH | LaTeX compilation, cross-ref consistency, notation/format checks |
| `FormalGate/DimensionalConsistency` | RESEARCH | CAS identity simplification, dimensional analysis, SMT consistency |

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
