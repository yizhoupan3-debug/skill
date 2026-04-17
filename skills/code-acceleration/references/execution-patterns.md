# Execution Patterns

Use this file when the acceleration plan is driven more by execution shape than by a single library swap. Keep the top-level `SKILL.md` focused on routing and decision order; use these notes for tactic selection and tradeoff checks.

## Decision order

Before adding workers or caches, classify the bottleneck:
- CPU-bound compute
- I/O-bound wait time
- memory pressure or cache locality
- serialization or copy overhead
- coordination overhead such as locks, IPC, or task scheduling

Prefer the simplest lever that removes the dominant cost:
1. better algorithm or access pattern
2. less copying or materialization
3. batching or pipelining
4. caching or incremental recomputation
5. vectorization, columnar execution, or JIT
6. concurrency or parallelism

## Parallelism

Use parallelism only when the work is large enough and independent enough to scale.

### CPU-bound work

Prefer first:
- algorithmic improvements
- NumPy, Polars, PyArrow, or other columnar/vectorized kernels
- Numba or native code when the hot loop can stay compatible

Consider processes when:
- each task has enough work to amortize startup and IPC cost
- inputs and outputs are not enormous to serialize
- state can stay partitioned with little cross-worker coordination

Be careful about:
- Python threads not helping CPU-bound work because of the GIL
- large object graphs that cost more to pickle than to compute
- skew where one worker gets the slow shard
- hidden shared-state contention

### I/O-bound work

Prefer first:
- async I/O
- connection reuse and pooling
- request batching
- prefetching and overlap between read, parse, and write stages

Be careful about:
- unbounded concurrency causing queueing collapse
- rate limits and backpressure
- increasing fan-out without reducing end-to-end latency

### Mixed workloads

Split the pipeline into stages and profile each stage separately.

Prefer:
- bounded producer-consumer queues
- explicit stage ownership
- stage-local batching
- overlap between I/O and compute only where downstream pressure is controlled

Reject:
- unbounded fan-out
- shared mutable state across stages
- designs that destroy deterministic ordering without benefit

## Batching and pipelining

Choose batching when fixed per-item overhead dominates.

Typical wins:
- fewer network round trips
- fewer parser or serializer invocations
- fewer database cursor transitions
- less scheduler overhead

Choose pipelining when work naturally flows through stages and one stage can run while another waits.

Typical wins:
- read-ahead while current chunk is computing
- encode next payload while current payload is in flight
- overlap disk, network, and compute without materializing the full dataset

Watch for:
- too-large batches increasing latency or memory spikes
- too-small batches failing to amortize overhead
- accidental reordering or duplicate processing

## Caching and incremental recomputation

Choose local caching when repeated work has stable inputs and a clear invalidation boundary.

Good fits:
- pure function memoization
- parsed schema or template reuse
- content-addressed artifact reuse
- incremental recomputation for append-only or partitioned data

Be careful about:
- stale results when invalidation is implicit
- caches masking correctness bugs
- optimizing warm-cache benchmarks while cold-start cost remains unacceptable

Always measure:
- cold behavior
- warm behavior
- invalidation or refresh cost

## Artifact-first resumability for long-running experiment loops

Use this pattern when repeated reruns are dominated by a small number of missing or stale shards rather than by one hot inner loop.

Prefer:
- choosing a stable work unit such as a shard, seed, fold, or report slice
- defining the required artifact set for a unit before calling it reusable
- rerunning only missing or stale units, then rebuilding the root summary from completed units
- writing aggregate summaries incrementally so partial progress is inspectable and recoverable

Be careful about:
- treating "directory exists" as proof that a unit is complete
- mixing outputs from incompatible effective configs or protocol revisions
- reusing summaries that were written before all required child artifacts landed
- hiding saved recomputation behind an unchanged wall-clock report

Validate:
- stale or partial artifacts are rejected
- resumed runs produce the same root summary as a clean full recompute
- the reported win includes reused work versus recomputed work

## Reducing data movement

Many slow paths are dominated by moving or reshaping data rather than computing on it.

Check for:
- repeated JSON encode or decode cycles
- pandas, Arrow, and Polars round-trips
- repeated conversion between Python objects and columnar arrays
- unnecessary cross-process transfer
- intermediate materialization that can stay lazy or streaming

Prefer:
- zero-copy or bytes-first paths where semantics permit
- late materialization
- keeping work inside one execution engine as long as possible

## Validation

When the execution shape changes, verify more than a single wall-clock number.

Measure as needed:
- wall time
- throughput
- p95 or p99 latency
- peak RSS
- worker scaling at multiple counts such as 1, 2, 4, and 8
- cold versus warm cache behavior

Also verify:
- ordering guarantees
- idempotency and retry safety
- null and type behavior
- whether the bottleneck merely moved to another stage
