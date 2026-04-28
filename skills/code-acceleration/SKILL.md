---
name: code-acceleration
description: Speed up existing code with measured rewrites, batching, caching, parallelism, and faster runtime or library choices.
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Speed up code with measured rewrites, batching, caching, and parallel execution
trigger_hints:
  - acceleration
  - profiling
  - hot path
  - batching
  - caching
  - pandas to polars
  - faster serializer
  - parallelize workload
metadata:
  version: "1.5.0"
  platforms: [codex]
  tags:
    - performance
    - optimization
    - acceleration
    - profiling
    - batching
    - caching
    - concurrency
    - multiprocessing
    - serialization
    - throughput
    - polars
    - pandas
    - duckdb
    - orjson
    - msgspec
    - numba
    - pyarrow
    - uvloop
    - hot-path
    - memory
risk: medium
source: local
---

# code-acceleration

This skill owns performance-first acceleration of existing code paths when the task is to make them materially faster, smaller, or more memory-efficient without changing the product goal.

## When to use

- The user wants an existing code path to run faster, use less memory, or handle larger inputs
- The likely fix is algorithmic, data-layout, streaming, or library substitution rather than feature work
- The bottleneck is in ETL, batch jobs, parsers, serializers, API hot paths, or data pipelines
- The user wants a measured library swap such as pandas to Polars, pandas to DuckDB, `json` to `orjson`, validation to `msgspec` or `pydantic-core`, or Python loops to vectorized execution
- The user wants to parallelize work, overlap I/O and compute, introduce batching, reduce copies and serialization, or add safe local caching or incremental recomputation
- The task is a long-running experiment, evaluation, or report pipeline where rerunning the full matrix is wasteful and the likely win is resumability, artifact reuse, or incremental summary materialization
- Best for requests like:
  - "帮我做代码加速"
  - "这段 pandas 能不能换成 polars"
  - "这个数据处理脚本太慢了，帮我优化"
  - "把这个热点路径换成更快的库"
  - "这段代码适合多线程还是多进程"
  - "这个任务能不能并行化或者做批处理"
  - "有没有缓存或增量计算的加速方案"

## Do not use

- The main task is frontend performance, Lighthouse, LCP, INP, CLS, or bundle size -> use `$performance-expert`
- The task is SQL query tuning, schema/index strategy, or EXPLAIN analysis -> use `$sql-pro`
- The task is load, stress, or soak benchmarking at service level -> use `$api-load-tester`
- The task is build, bundler, or compile-speed tuning -> use `$build-tooling` or `$latex-compile-acceleration`
- The task is ML training throughput or hardware-specific training stability -> use `$ai-research` or `$mac-memory-management`
- The task is general language implementation where performance is secondary -> use the language owner such as `$python-pro`, `$typescript-pro`, `$go-pro`, or `$rust-pro`
- The main problem is datastore, cache invalidation, queue semantics, retries, or source-of-truth correctness -> use `$datastore-cache-queue`

## Task ownership and boundaries

This skill owns:
- measurement-first acceleration work on existing code
- hot-path identification and cost-model reasoning
- faster-library substitutions when semantics can be preserved
- algorithmic and data-flow rewrites that reduce asymptotic or constant-factor cost
- memory-aware rewrites such as streaming, chunking, zero-copy, and columnar execution
- batching, pipelining, prefetching, and overlap between I/O and compute
- safe local caching or incremental recomputation when the invalidation boundary is explicit
- artifact-level reuse, resumable shard or seed execution, and aggregate rebuilding from completed outputs
- reducing serialization, copying, materialization, and cross-process transfer overhead
- concurrency and parallelism when the workload can actually scale
- before-and-after verification for correctness, wall time, and memory

This skill does not own:
- user-visible web performance and delivery metrics
- pure database tuning
- platform build loops
- hardware provisioning or infra sizing
- benchmark theater without code changes
- correctness redesign of distributed cache, queue, or worker semantics

## Research companion mode

- Research execution owners such as [`$autoresearch`](../autoresearch/SKILL.md) and [`$ai-research`](../ai-research/SKILL.md) should proactively check this skill when they write or revise experiment code, data pipelines, evaluation harnesses, inference paths, or agent loops that may become throughput or memory bottlenecks.
- Treat memory efficiency as part of this skill's default surface for generic code paths; on Apple Silicon, MPS, or unified-memory bottlenecks, use [`$mac-memory-management`](../mac-memory-management/SKILL.md) as the primary runtime companion before escalating to this skill.
- Companion routing should happen before expensive runs when the execution risk is obvious, not only after user-visible slowness.

## Required workflow

1. Confirm the task shape:
   - object: existing code path, pipeline, parser, service hot path, batch job
   - action: profile, accelerate, substitute libraries, refactor hot path
   - constraints: correctness parity, runtime target, dependency policy, memory budget, allowed behavior drift
   - deliverable: code change plus measured before/after evidence
2. Measure before changing:
   - identify the hot path with profiler output, timings, row counts, or representative benchmarks
   - use the right tool for the question: `timeit` for small snippets, `pyperf` for repeatable comparisons, `cProfile` for call attribution, and `tracemalloc` when memory is the concern
   - estimate whether the bottleneck is algorithmic, I/O, allocation, serialization, interpreter overhead, cache reuse, coordination overhead, or cache/memory layout
   - map the stable recomputation unit and artifact reuse boundary before changing execution shape
   - if the workload is unstable or cannot be reproduced on representative input, stop and say what evidence is missing
3. Choose the highest-payoff class of win first:
   - better algorithm or access pattern
   - better data layout or memory locality
   - library/runtime substitution
   - engine-specific planning and physical-layout tuning
   - batching / streaming / chunking / pipelining
   - local caching or incremental recomputation
   - resumability / partial reruns / incremental materialization when repeated partial reruns dominate cost
   - vectorization / columnar execution
   - concurrency or parallelism only when the bottleneck actually benefits
   - native offload or JIT only as a non-default escalation after higher-level wins stall
4. Preserve semantics:
   - check null behavior, ordering, dtypes, timezone handling, index semantics, and error behavior
   - check idempotency, retry safety, and ordering expectations before changing execution shape
   - call out any intentional behavior changes before landing them
5. Re-measure after the change:
   - compare wall-clock time and memory on the same representative input
   - report both the speedup and the remaining bottleneck
   - when the change introduces workers, batching, caching, or pipeline overlap, also report throughput and scaling behavior if feasible
   - report reused work versus recomputed work when resumability or artifact reuse is part of the win
   - do not claim a win from a microbenchmark that does not resemble the real workload

## Execution-pattern guidance

- For common substitution patterns and migration traps, read [references/library-swaps.md](./references/library-swaps.md).
- For DuckDB physical-layout, thread, and I/O tuning, read [references/duckdb-tuning.md](./references/duckdb-tuning.md).
- For parallelism, batching, pipelining, caching, and validation heuristics, read [references/execution-patterns.md](./references/execution-patterns.md).
- For long-running experiment loops, see the artifact-first resumability pattern in [references/execution-patterns.md](./references/execution-patterns.md).
- Prefer structural wins before adding coordination machinery.
- Prefer parallelism only after checking whether the real bottleneck is CPU, I/O, memory bandwidth, serialization, or lock contention.
- Prefer reducing data movement and intermediate materialization before adding workers.

## Library-swap guidance

- Prefer Polars or DuckDB when the workload is dominated by column scans, joins, group-bys, or SQL-shaped analytics.
- For DuckDB, treat engine tuning as its own mode: check query shape, file layout, thread policy, and I/O behavior before assuming a simple library swap is enough.
- Prefer `orjson`, `msgspec`, or `pydantic-core` when parsing, validation, or serialization is a measured hot path.
- Prefer `pyarrow.compute`, NumPy vectorization, or Numba when Python interpreter overhead is the bottleneck on homogeneous data.
- Prefer `uvloop` only after request-path measurement shows event-loop cost is material.
- Prefer streaming or chunked approaches when the data volume exceeds memory headroom.
- Reject a faster library swap if correctness, ecosystem compatibility, or maintenance cost outweigh the measured win.

## DuckDB tuning mode

- Use DuckDB tuning mode when the workload already lives in DuckDB or the likely win depends on query planning, thread policy, Parquet layout, or remote/local I/O behavior rather than a simple engine swap.
- Check whether the bottleneck is query shape, repeated small queries, excessive round-trips to pandas, row-group parallelism limits, or synchronous file I/O before rewriting surrounding code.
- Prefer keeping joins, filters, and aggregations inside DuckDB until the final handoff.

## Parallelism decision gate

- For CPU-bound work, prefer algorithmic wins, vectorization, columnar execution, or native/JIT acceleration before adding Python threads.
- For CPU-bound work that still needs parallelism, prefer processes only when the task granularity and IPC cost leave room for real speedup.
- For I/O-bound work, prefer async I/O, batching, connection reuse, prefetching, and pipelined stage overlap before adding more workers.
- For mixed workloads, split stages explicitly and use bounded queues or backpressure rather than unbounded fan-out.
- Reject parallelism when ordering, shared mutable state, lock contention, or serialization overhead is likely to dominate.

## Verification expectations

- Measure cold and warm behavior when caching or incremental recomputation is introduced.
- Measure speedup across multiple worker counts when parallelism is introduced; do not stop at a single before or after number.
- Check wall time, throughput, peak memory, and tail latency when they matter to the workload shape.
- Validate stale-artifact rejection and resume correctness when reuse or partial reruns are introduced.
- Validate that root or aggregate summaries remain correct when written incrementally from completed shards.
- Report whether the optimization shifted the bottleneck instead of removing it.

## Hard constraints

- Do not optimize blind; collect at least lightweight timing evidence first.
- Do not replace a library without checking semantic mismatches.
- Do not report wins from synthetic microbenchmarks that do not resemble the real workload.
- Prefer structural wins before micro-optimizations.
- Do not default to JIT or experimental runtime features while algorithm, planner, layout, batching, or vectorization wins are still available.
- Do not add concurrency before checking coordination, locking, IPC, and data-transfer cost.
- Do not add caching unless the invalidation boundary and source of truth are explicit.
- Do not reuse artifacts without an explicit completeness contract for the shard, seed, or summary being reused.
- Do not validate behavior or speed against stale pre-run config when runtime produces an effective config that can diverge from the input config.
- If the performance issue is not reproducible, say so and define the missing evidence.
- For critical acceleration work, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "帮我把这段 pandas 处理换成 polars。"
- "这个热点路径太慢了，换更快的库。"
- "这段代码适合多线程、多进程还是异步批处理？"
- "这个流水线能不能边读边算，少做中间物化？"
- "想做缓存或增量计算来加速这段处理。"
- "这个 5-seed rerun 太贵，能不能只跑缺失 seeds 并重建总表？"
- "Use $code-acceleration to speed up this ETL job and verify before/after timings."
- "强制进行代码加速审计 / 检查热点定位与加速结果。"
