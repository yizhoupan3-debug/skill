# Library Swaps

Use this file when the acceleration plan is dominated by a library substitution rather than a pure algorithm change. If the task is not already measured, do that first and postpone the swap decision.

## Evidence bar

Before proposing a swap, capture:
- the hot path and its input shape
- a representative baseline on real or replayed data
- the correctness invariants that must not drift
- wall-clock and, when relevant, memory usage

Benchmarking guidance:
- use `timeit` for small, isolated snippets
- use `pyperf` for repeatable benchmark comparisons
- use `cProfile` or `profiling.tracing` to attribute call costs
- use `tracemalloc` when allocation pressure or leaks matter
- avoid claiming victory from a toy benchmark that does not resemble production input

## Pandas -> Polars

Choose this when:
- the workload is tabular and column-oriented
- the pipeline is dominated by filter, select, join, group-by, sort, window, or scan steps
- lazy execution can fuse work and avoid materializing intermediates
- Python row loops and `DataFrame.apply` are the current bottleneck

Be careful about:
- pandas index-heavy semantics that do not map cleanly to Polars
- Python UDFs that keep work in Python and erase the performance gain
- dtype differences, especially categoricals, datetimes, timezone handling, and nulls
- output ordering assumptions after joins, group-bys, or lazy optimization
- ecosystem gaps when downstream code expects pandas-specific objects

Migration checklist:
1. Replace eager CSV or Parquet reads with `scan_*` where appropriate.
2. Collapse chained intermediate frames into one lazy pipeline when possible.
3. Rewrite row-wise `apply` into expression APIs.
4. Re-check null semantics, sorting, and type coercion.
5. Benchmark on representative data volume, not toy data.
6. Keep the result in Polars only if downstream code does not require pandas-specific behavior.

## Pandas -> DuckDB

Choose this when:
- large joins or aggregations dominate
- the workload is naturally SQL-shaped
- datasets are large enough that query planning and columnar execution matter
- multiple file inputs need in-process analytical joins

Be careful about:
- repeated round-trips between pandas and DuckDB that erase the win
- hidden materialization of intermediate results
- subtle SQL vs pandas null and ordering behavior

Why this often wins:
- DuckDB is an in-process SQL OLAP engine built on a fast columnar storage engine.
- It can query files directly and spill to disk, which makes it a good fit when the pandas path is pushing memory.

Migration checklist:
1. Keep joins, filters, and aggregations inside one SQL query when possible.
2. Avoid converting back to pandas until the final handoff.
3. Verify null grouping, ordering, and type coercion against the original path.
4. If the win depends on Parquet layout, thread count, or remote-file behavior, switch to [DuckDB tuning](./duckdb-tuning.md) instead of treating this as a pure library swap.

## Python stdlib json -> orjson

Choose this when:
- serialization or parsing is a measurable hot path
- JSON encode or decode shows up prominently in profiling
- the surrounding code can accept `bytes`-first behavior where applicable

Be careful about:
- behavior differences in datetime handling and custom object serialization
- integration points that require `str` rather than `bytes`

Why this often wins:
- `orjson.dumps()` returns `bytes`, not `str`.
- Passing `bytes` or `memoryview` directly to `orjson.loads()` avoids an unnecessary UTF-8 decode step and reduces latency and memory use.
- `orjson` serializes dataclasses, datetimes, numpy arrays, and UUIDs natively.

Migration checklist:
1. Audit every caller that expects text instead of bytes.
2. Check datetime, key sorting, non-string keys, and custom default handling.
3. Keep an eye on invalid-JSON behavior, especially around NaN and Infinity.

## json / pydantic / dataclasses -> msgspec

Choose this when:
- structured messages or API payloads are a measurable hot path
- the workload needs both schema validation and fast encode/decode
- you can move to explicit typed structs instead of ad-hoc dict handling

Be careful about:
- migration cost if the surrounding framework expects Pydantic-specific models
- custom validators or serialization hooks that need rewriting

Why this often wins:
- `msgspec` combines fast serialization with typed validation during deserialization.
- It is a better fit when message shapes are stable and you can model them explicitly.

Migration checklist:
1. Replace ad-hoc dicts with typed structs only where the schema is stable.
2. Confirm downstream tooling does not depend on Pydantic model APIs.
3. Rebuild any custom validation logic that the new schema does not express directly.

## Pydantic v1 / high-overhead validation -> Pydantic v2 / pydantic-core

Choose this when:
- request validation, settings parsing, or model serialization is a measurable bottleneck
- the codebase is still paying v1-era validation costs on hot paths

Be careful about:
- validator API differences between major versions
- behavior drift in coercion, serialization, and custom field logic

Why this often wins:
- `pydantic-core` moves most validation into Rust-backed internals.
- This is usually the right upgrade when validation remains necessary but the current path is dominated by Pydantic overhead.

Migration checklist:
1. Audit validator and serializer hooks before switching.
2. Compare coercion, defaults, and error text on representative payloads.
3. Keep the upgrade local to the hot path if the whole app does not need it.

## Arrow-native transformations -> pyarrow.compute

Choose this when:
- data is already in Arrow arrays or tables
- the pipeline needs filter, cast, arithmetic, string, or temporal kernels without pandas materialization
- the work can stay in Arrow's columnar representation instead of bouncing through Python objects

Be careful about:
- needless pandas round-trips
- loss of benefit if downstream code immediately converts back to Python objects

## Python loops -> vectorized or columnar execution

Choose this when:
- the bottleneck is Python interpreter overhead on large homogeneous collections
- the operation can be expressed as array or expression transformations

Prefer:
- NumPy for numeric arrays and dense math
- Polars expressions for dataframe-style transformations
- PyArrow compute when arrow-native data already exists

Avoid when:
- the workload is tiny
- the logic is branch-heavy and hard to vectorize correctly
- the vectorized rewrite obscures correctness without meaningful payoff

## Non-default escalation: Python loops -> Numba JIT

Choose this when:
- loops are numeric, array-oriented, and still awkward to vectorize cleanly
- profiling shows interpreter overhead on repeated numeric kernels
- you can stay in `nopython` mode rather than falling back to object mode

Be careful about:
- object-heavy Python code that Numba cannot optimize well
- unsupported APIs falling back to object mode and erasing the win
- treating JIT as a default workflow step before algorithm, layout, batching, vectorization, or planner wins are exhausted

## asyncio default loop / parser -> uvloop / lower-overhead runtime pieces

Choose this when:
- API or network I/O profiling shows meaningful event-loop or protocol-parser cost
- the workload is latency-sensitive and already otherwise optimized
- the target platform supports a drop-in `asyncio` event loop replacement

Be careful about:
- platform compatibility assumptions
- claiming wins before measuring real request paths

## Validation rules

- Keep a representative benchmark input checked in or documented.
- Compare correctness before performance: row counts, aggregates, null counts, ordering assumptions, type expectations, and error behavior.
- Record both wall-clock impact and memory impact when relevant.
- If the swap changes semantics or dependencies in a visible way, state that explicitly in the final writeup.
