# DuckDB Tuning

Use this reference when the acceleration plan depends on DuckDB itself, not only on
switching into DuckDB from another dataframe engine.

## When to use this mode

Choose DuckDB tuning mode when one or more of these are true:
- the workload already runs in DuckDB
- large Parquet or multi-file scans dominate
- joins and aggregations are already SQL-shaped
- repeated small queries are slower than expected
- threading, row-group layout, or remote-file reads appear to cap throughput

## Tuning order

1. keep the work inside DuckDB as long as possible
2. check query shape and unnecessary round-trips
3. check file layout and row-group parallelism
4. check thread policy
5. check local versus remote I/O behavior

## Query-shape checks

Look for:
- repeated conversions back to pandas or Python objects
- many small queries where one larger query would do
- repeated parsing or planning of similar statements
- intermediate materialization outside DuckDB

Prefer:
- one SQL query for join/filter/aggregate pipelines
- prepared statements or batched execution for repeated small queries
- delaying pandas conversion until final handoff

## Physical-layout checks

DuckDB parallelism and scan efficiency depend on the underlying layout.

Check:
- Parquet row-group size and count
- whether the dataset has enough row groups to feed the configured threads
- whether file partitioning creates too many tiny files or too little parallel work

Prefer:
- file and row-group layouts that expose enough scan parallelism
- avoiding very small files that increase overhead
- validating whether layout changes beat query rewrites

## Thread tuning

Check:
- whether thread count is too low to use available scan parallelism
- whether thread count is too high and causes coordination overhead
- whether Python-side threading is adding overhead around DuckDB rather than helping it

Prefer:
- measuring a few thread-count settings instead of assuming "more threads is faster"
- thread-local cursors when Python threads must share one connection pattern

## I/O tuning

Check:
- local disk versus remote object-store reads
- whether remote scans are effectively synchronous or latency-bound
- repeated reads of the same files without reuse

Prefer:
- reducing repeated remote reads
- staging or caching inputs when network latency dominates
- measuring remote and local paths separately

## Validation

When claiming a DuckDB win, record:
- the query or workload shape
- file format and layout assumptions
- thread count used
- local or remote input path
- before/after wall time and, when relevant, memory use
